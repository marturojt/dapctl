use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc::Sender;
use std::time::Instant;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::config::{TranscodeRule, Verify};
use crate::diff::{EntryKind, Plan};
use crate::transfer::manifest::{Manifest, ManifestEntry, State};
use crate::transfer::ProgressEvent;

/// Read+write buffer: 1 MiB gives ~80 progress-bar updates for an 86 MB FLAC.
const BUF: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum SyncMode {
    /// Add/overwrite only; never delete orphans.
    Additive,
    /// Add/overwrite + delete orphans.
    Mirror,
}

pub struct Options {
    pub dry_run: bool,
    pub mode: SyncMode,
    pub verify: Verify,
    pub run_id: String,
    pub manifest_dir: Utf8PathBuf,
    /// When set, progress events are sent here and indicatif bars are hidden.
    pub progress_tx: Option<Sender<ProgressEvent>>,
    /// When set, files with a matching `transcode_from` in the plan are run
    /// through ffmpeg instead of being copied directly.
    pub transcode: Option<TranscodeOpts>,
}

pub struct TranscodeOpts {
    pub rules: Vec<TranscodeRule>,
    pub cache: crate::transcode::Cache,
}

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub copied: usize,
    pub deleted: usize,
    pub failed: usize,
    pub bytes_written: u64,
    pub elapsed_secs: f64,
}

pub fn execute(
    plan: &Plan,
    src_root: &Utf8Path,
    dst_root: &Utf8Path,
    opts: &Options,
) -> anyhow::Result<Stats> {
    let to_copy: Vec<_> = plan
        .entries
        .iter()
        .filter(|e| matches!(e.kind, EntryKind::New | EntryKind::Modified))
        .collect();

    let to_delete: Vec<_> = match opts.mode {
        SyncMode::Mirror => plan
            .entries
            .iter()
            .filter(|e| e.kind == EntryKind::Orphan)
            .collect(),
        SyncMode::Additive => Vec::new(),
    };

    if opts.dry_run {
        return Ok(dry_run_stats(plan, opts.mode));
    }

    let transfer_bytes: u64 = to_copy.iter().map(|e| e.size_bytes).sum();
    let start = Instant::now();

    // ── Manifest ──────────────────────────────────────────────────────────
    let manifest_entries: Vec<ManifestEntry> = to_copy
        .iter()
        .map(|e| ManifestEntry {
            path: e.path.clone(),
            size_bytes: e.size_bytes,
            state: State::Pending,
            err: None,
        })
        .collect();
    let mut manifest = Manifest::create(
        &opts.run_id,
        "",
        &opts.manifest_dir,
        &manifest_entries,
    )?;

    // ── Progress bars (hidden when TUI consumes events via channel) ────────
    let tui_mode = opts.progress_tx.is_some();
    let _mp: Option<MultiProgress>;
    let overall: ProgressBar;
    let file_bar: ProgressBar;

    if tui_mode {
        overall = ProgressBar::hidden();
        file_bar = ProgressBar::hidden();
        _mp = None;
    } else {
        let mp = MultiProgress::new();
        let o = mp.add(ProgressBar::new(transfer_bytes));
        o.set_style(
            ProgressStyle::with_template(
                "  Overall  {wide_bar:.cyan/blue}  {percent:>3}%  {bytes}/{total_bytes}  {binary_bytes_per_sec}  ETA {eta}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        let f = mp.add(ProgressBar::new(0));
        f.set_style(
            ProgressStyle::with_template(
                "  Current  {wide_bar:.green/white}  {percent:>3}%  {bytes}/{total_bytes}\n           {msg}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        overall = o;
        file_bar = f;
        _mp = Some(mp);
    }

    let mut stats = Stats::default();

    // ── Copy loop ──────────────────────────────────────────────────────────
    for entry in &to_copy {
        let dst = dst_root.join(&entry.path);
        let tmp = tmp_path(&dst);

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {parent}"))?;
        }

        let _ = manifest.update(&ManifestEntry {
            path: entry.path.clone(),
            size_bytes: entry.size_bytes,
            state: State::InProgress,
            err: None,
        });

        tracing::debug!(event = "xfer_start", path = %entry.path, bytes = entry.size_bytes);

        if let Some(ref tx) = opts.progress_tx {
            let _ = tx.send(ProgressEvent::FileStart {
                path: entry.path.to_string(),
                bytes: entry.size_bytes,
            });
        }

        // Decide copy strategy: direct copy or transcode via ffmpeg.
        let copy_result = if let (Some(from_ext), Some(ref tc)) =
            (&entry.transcode_from, &opts.transcode)
        {
            // Locate the actual source file using the original extension.
            let src_path = entry.path.with_extension(from_ext);
            let actual_src = src_root.join(&src_path);
            let rule = tc.rules.iter()
                .find(|r| r.from.to_lowercase() == from_ext.to_lowercase());

            if let Some(rule) = rule {
                file_bar.set_length(0); // indeterminate during transcode
                file_bar.set_message(format!(
                    "[TC] {}",
                    truncate_path(&entry.path, 65)
                ));
                do_transcode(&actual_src, &dst, &tmp, rule, tc)
            } else {
                // Rule disappeared at runtime — fall back to direct copy.
                let src = src_root.join(&entry.path);
                file_bar.set_length(entry.size_bytes);
                file_bar.set_position(0);
                file_bar.set_message(truncate_path(&entry.path, 70));
                copy_with_progress(&src, &tmp, &file_bar, &overall, opts.progress_tx.as_ref())
            }
        } else {
            let src = src_root.join(&entry.path);
            file_bar.set_length(entry.size_bytes);
            file_bar.set_position(0);
            file_bar.set_message(truncate_path(&entry.path, 70));
            copy_with_progress(&src, &tmp, &file_bar, &overall, opts.progress_tx.as_ref())
        };

        // ── Post-copy: rename, preserve mtime, verify ─────────────────────
        let actual_src_for_mtime = if let Some(from_ext) = &entry.transcode_from {
            src_root.join(entry.path.with_extension(from_ext))
        } else {
            src_root.join(&entry.path)
        };
        let is_transcoded = entry.transcode_from.is_some();

        match copy_result {
            Ok(written) => {
                if dst.exists() {
                    std::fs::remove_file(&dst)
                        .with_context(|| format!("cannot remove old {dst}"))?;
                }
                std::fs::rename(&tmp, &dst)
                    .with_context(|| format!("cannot rename tmp → {dst}"))?;

                preserve_mtime(&actual_src_for_mtime, &dst);

                // Transcoded files verify that the output exists and is non-empty.
                // Format-agnostic checksum comparison makes no sense across formats.
                let verified = if is_transcoded {
                    dst.metadata().map_or(false, |m| m.len() > 0)
                } else {
                    match opts.verify {
                        Verify::None => true,
                        Verify::SizeMtime => crate::transfer::verify::size_mtime(
                            &actual_src_for_mtime,
                            &dst,
                        )
                        .unwrap_or(false),
                        Verify::Checksum => crate::transfer::verify::checksum(
                            &actual_src_for_mtime,
                            &dst,
                        )
                        .unwrap_or(false),
                    }
                };

                if verified {
                    stats.copied += 1;
                    stats.bytes_written += written;
                    tracing::info!(event = "xfer_done", path = %entry.path, bytes = written);
                    let _ = manifest.update(&ManifestEntry {
                        path: entry.path.clone(),
                        size_bytes: entry.size_bytes,
                        state: State::Done,
                        err: None,
                    });
                    if let Some(ref tx) = opts.progress_tx {
                        let _ = tx.send(ProgressEvent::FileDone {
                            path: entry.path.to_string(),
                            bytes: written,
                        });
                    }
                } else {
                    stats.failed += 1;
                    tracing::warn!(event = "verify_fail", path = %entry.path);
                    let _ = manifest.update(&ManifestEntry {
                        path: entry.path.clone(),
                        size_bytes: entry.size_bytes,
                        state: State::Failed,
                        err: Some("verify mismatch".to_owned()),
                    });
                    if let Some(ref tx) = opts.progress_tx {
                        let _ = tx.send(ProgressEvent::FileFail {
                            path: entry.path.to_string(),
                            err: "verify mismatch".to_owned(),
                        });
                    }
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                stats.failed += 1;
                let msg = e.to_string();
                tracing::error!(event = "xfer_fail", path = %entry.path, err = %msg);
                let _ = manifest.update(&ManifestEntry {
                    path: entry.path.clone(),
                    size_bytes: entry.size_bytes,
                    state: State::Failed,
                    err: Some(msg.clone()),
                });
                if let Some(ref tx) = opts.progress_tx {
                    let _ = tx.send(ProgressEvent::FileFail {
                        path: entry.path.to_string(),
                        err: msg,
                    });
                }
            }
        }
    }

    if transfer_bytes > 0 {
        overall.finish_and_clear();
        file_bar.finish_and_clear();
    }

    // ── Delete orphans ─────────────────────────────────────────────────────
    for entry in &to_delete {
        let dst = dst_root.join(&entry.path);
        match std::fs::remove_file(&dst) {
            Ok(()) => {
                stats.deleted += 1;
                tracing::info!(event = "delete", path = %entry.path);
                if let Some(ref tx) = opts.progress_tx {
                    let _ = tx.send(ProgressEvent::DeleteDone {
                        path: entry.path.to_string(),
                    });
                }
            }
            Err(e) => {
                stats.failed += 1;
                tracing::error!(event = "delete_fail", path = %entry.path, err = %e);
                if let Some(ref tx) = opts.progress_tx {
                    let _ = tx.send(ProgressEvent::FileFail {
                        path: entry.path.to_string(),
                        err: e.to_string(),
                    });
                }
            }
        }
    }

    stats.elapsed_secs = start.elapsed().as_secs_f64();

    if let Some(ref tx) = opts.progress_tx {
        let _ = tx.send(ProgressEvent::Finish { stats: stats.clone() });
    }

    Ok(stats)
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn copy_with_progress(
    src: &Utf8Path,
    dst: &Utf8Path,
    file_bar: &ProgressBar,
    overall: &ProgressBar,
    progress_tx: Option<&Sender<ProgressEvent>>,
) -> anyhow::Result<u64> {
    let src_file = std::fs::File::open(src.as_std_path())
        .with_context(|| format!("cannot open {src}"))?;
    let dst_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dst.as_std_path())
        .with_context(|| format!("cannot create {dst}"))?;

    let mut reader = BufReader::with_capacity(BUF, src_file);
    let mut writer = BufWriter::with_capacity(BUF, dst_file);
    let mut buf = vec![0u8; BUF];
    let mut total = 0u64;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        total += n as u64;
        file_bar.inc(n as u64);
        overall.inc(n as u64);
        if let Some(tx) = progress_tx {
            let _ = tx.send(ProgressEvent::FileProgress { bytes: n as u64 });
        }
    }

    let file = writer.into_inner().context("flush error")?;
    file.sync_data()?;

    Ok(total)
}

/// Run ffmpeg to transcode `src` into `tmp`, using the rule's params.
/// If the cache has a valid entry it is copied from cache instead, avoiding
/// a re-transcode. The result is written to `tmp` (the caller renames it to `dst`).
/// Returns the number of bytes written.
fn do_transcode(
    src: &Utf8Path,
    dst: &Utf8Path,
    tmp: &Utf8Path,
    rule: &TranscodeRule,
    tc: &TranscodeOpts,
) -> anyhow::Result<u64> {
    let to_ext = rule.to.to_lowercase();

    // Check cache.
    let key = crate::transcode::cache_key(src, &rule.params)
        .with_context(|| format!("cannot hash {src} for transcode cache"))?;

    if let Some(cached) = tc.cache.get(&key, &to_ext) {
        tracing::debug!(event = "transcode_cache_hit", path = %dst);
        std::fs::copy(cached.as_std_path(), tmp.as_std_path())
            .with_context(|| format!("cannot copy cached transcode to {tmp}"))?;
    } else {
        tracing::info!(event = "transcode_start", src = %src, dst = %dst);
        crate::transcode::transcode(src, tmp, rule)
            .with_context(|| format!("transcode {src} → {dst}"))?;
        // Store in cache (best-effort; failure is non-fatal).
        if let Err(e) = tc.cache.store(&key, &to_ext, tmp) {
            tracing::warn!(event = "cache_store_fail", err = %e);
        }
    }

    Ok(tmp.metadata().map_or(0, |m| m.len()))
}

/// Walk `src_root` + `dst_root` and for every file that exists in both with
/// matching size, set the destination mtime to match the source.
pub fn repair_dest_mtimes(src_root: &Utf8Path, dst_root: &Utf8Path) -> usize {
    use std::fs::FileTimes;
    use walkdir::WalkDir;

    let mut count = 0usize;

    for entry in WalkDir::new(src_root.as_std_path())
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let src_path = entry.path();
        let Ok(rel) = src_path.strip_prefix(src_root.as_std_path()) else { continue };
        let dst_path = dst_root.as_std_path().join(rel);

        let Ok(src_meta) = std::fs::metadata(src_path) else { continue };
        let Ok(dst_meta) = std::fs::metadata(&dst_path) else { continue };

        if src_meta.len() != dst_meta.len() {
            continue;
        }

        let Ok(src_mtime) = src_meta.modified() else { continue };
        let Ok(dst_mtime) = dst_meta.modified() else { continue };

        let to_ns = |t: std::time::SystemTime| -> i128 {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as i128)
                .unwrap_or(0)
        };

        if (to_ns(src_mtime) - to_ns(dst_mtime)).abs() <= 2_000_000_000 {
            continue;
        }

        let Ok(f) = std::fs::OpenOptions::new().write(true).open(&dst_path) else { continue };
        if f.set_times(FileTimes::new().set_modified(src_mtime)).is_ok() {
            count += 1;
        }
    }

    count
}

fn preserve_mtime(src: &Utf8Path, dst: &Utf8Path) {
    use std::fs::FileTimes;
    let Ok(meta) = std::fs::metadata(src.as_std_path()) else { return };
    let Ok(mtime) = meta.modified() else { return };
    let Ok(f) = std::fs::OpenOptions::new().write(true).open(dst.as_std_path()) else { return };
    let _ = f.set_times(FileTimes::new().set_modified(mtime));
}

fn tmp_path(dst: &Utf8Path) -> Utf8PathBuf {
    let name = dst.file_name().unwrap_or("file");
    let tmp_name = format!("{name}.dapctl-tmp");
    dst.parent()
        .map(|p| p.join(&tmp_name))
        .unwrap_or_else(|| Utf8PathBuf::from(tmp_name))
}

fn truncate_path(p: &Utf8Path, max: usize) -> String {
    let s = p.as_str();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_owned();
    }
    let tail: String = chars[chars.len().saturating_sub(max.saturating_sub(1))..].iter().collect();
    format!("…{tail}")
}

fn dry_run_stats(plan: &Plan, mode: SyncMode) -> Stats {
    let deleted = match mode {
        SyncMode::Mirror => plan.count(EntryKind::Orphan),
        SyncMode::Additive => 0,
    };
    Stats {
        copied: plan.count(EntryKind::New) + plan.count(EntryKind::Modified),
        deleted,
        failed: 0,
        bytes_written: plan.transfer_bytes(),
        elapsed_secs: 0.0,
    }
}
