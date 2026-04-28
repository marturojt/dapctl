use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;
use walkdir::WalkDir;

use crate::config::{Filters, TranscodeRule};
use crate::transfer::verify::hash_file;

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to the walk root, with `/` separators (platform-independent).
    /// For transcoded files this path already uses the *target* extension.
    pub rel: Utf8PathBuf,
    pub size: u64,
    /// Modification time as nanoseconds since UNIX epoch. 0 if unavailable.
    pub mtime_ns: i128,
    /// blake3 hash, populated only when the caller requests `compute_hashes = true`.
    pub hash: Option<blake3::Hash>,
    /// When `Some`, the file's extension was projected from this source extension
    /// (e.g. `"dsf"`) to a target extension (e.g. `"flac"`) by a transcode rule.
    pub src_ext: Option<String>,
}

/// Walk `root` recursively, applying exclusion, inclusion, tag, and transcode filters.
///
/// - `compute_hashes`: compute blake3 per entry; set only for `Verify::Checksum`.
/// - `tag_filters`: read audio headers via lofty when any tag filter is active.
/// - `transcode_rules`: when non-empty, source files whose extension matches a rule
///   have their `rel` path projected to the target extension (e.g. `song.dsf` →
///   `song.flac`). The original extension is recorded in `Entry::src_ext` so the
///   executor can locate the actual source file. Pass `&[]` for the destination walk.
///
/// Returns entries sorted by `rel` path for O(n) merge-join in `compare`.
pub fn walk(
    root: &Utf8Path,
    exclude: &GlobSet,
    include: Option<&GlobSet>,
    compute_hashes: bool,
    tag_filters: &Filters,
    transcode_rules: &[TranscodeRule],
) -> anyhow::Result<Vec<Entry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let use_tag_filters = tag_filters_active(tag_filters);
    let mut entries = Vec::new();

    for result in WalkDir::new(root).follow_links(false) {
        let de = result.with_context(|| format!("error walking {root}"))?;

        if !de.file_type().is_file() {
            continue;
        }

        let abs = de.path();
        let rel_os = abs
            .strip_prefix(root.as_std_path())
            .with_context(|| format!("{} is not under {root}", abs.display()))?;
        let rel_str = rel_os.to_string_lossy().replace('\\', "/");
        let mut rel = Utf8PathBuf::from(&rel_str);

        if exclude.is_match(&rel_str) {
            continue;
        }
        if let Some(inc) = include {
            if !inc.is_match(&rel_str) {
                continue;
            }
        }

        if use_tag_filters && !tag_matches(abs, tag_filters) {
            continue;
        }

        let meta = de.metadata()?;
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i128)
            .unwrap_or(0);

        // Transcode extension projection: done AFTER glob/tag filtering so that
        // include_globs apply to the source extension (e.g. "**/*.dsf"), not the
        // projected target extension.
        let src_ext = project_extension(&mut rel, &rel_str, transcode_rules);

        let abs_utf8 = root.join(if src_ext.is_some() {
            // Hash the original source file (not the projected path).
            Utf8PathBuf::from(&rel_str)
        } else {
            rel.clone()
        });
        let hash = if compute_hashes {
            hash_file(&abs_utf8).ok()
        } else {
            None
        };

        entries.push(Entry {
            rel,
            size: meta.len(),
            mtime_ns,
            hash,
            src_ext,
        });
    }

    entries.sort_unstable_by(|a, b| a.rel.cmp(&b.rel));
    Ok(entries)
}

// ── Transcode helpers ──────────────────────────────────────────────────────────

/// If `rel_str` matches a transcode rule, update `rel` to use the target extension
/// and return the original extension. Otherwise return `None`.
fn project_extension(
    rel: &mut Utf8PathBuf,
    rel_str: &str,
    rules: &[TranscodeRule],
) -> Option<String> {
    let src_ext_lc = rel_str.rsplit('.').next()?.to_lowercase();
    let rule = rules.iter().find(|r| r.from.to_lowercase() == src_ext_lc)?;
    let stem = rel_str.rsplit_once('.').map_or(rel_str, |(s, _)| s);
    *rel = Utf8PathBuf::from(format!("{}.{}", stem, rule.to.to_lowercase()));
    Some(src_ext_lc)
}

// ── Tag filter helpers ─────────────────────────────────────────────────────────

fn tag_filters_active(f: &Filters) -> bool {
    !f.include_artists.is_empty()
        || !f.exclude_artists.is_empty()
        || !f.include_genres.is_empty()
        || !f.exclude_genres.is_empty()
        || f.min_sample_rate_hz.is_some()
        || f.max_sample_rate_hz.is_some()
        || f.min_bit_depth.is_some()
}

/// Returns `true` if the file passes all configured tag filters.
/// On lofty parse error the file passes — we never silently drop files we can't inspect.
fn tag_matches(path: &std::path::Path, f: &Filters) -> bool {
    use lofty::prelude::{Accessor, AudioFile, TaggedFileExt};

    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return true,
    };

    let props = tagged.properties();

    if let Some(min) = f.min_sample_rate_hz {
        match props.sample_rate() {
            Some(sr) if sr >= min => {}
            _ => return false,
        }
    }
    if let Some(max) = f.max_sample_rate_hz {
        if let Some(sr) = props.sample_rate() {
            if sr > max {
                return false;
            }
        }
    }
    if let Some(min_bd) = f.min_bit_depth {
        match props.bit_depth() {
            Some(bd) if bd >= min_bd => {}
            _ => return false,
        }
    }

    if f.include_artists.is_empty()
        && f.exclude_artists.is_empty()
        && f.include_genres.is_empty()
        && f.exclude_genres.is_empty()
    {
        return true;
    }

    let tag = tagged.primary_tag();
    let artist_lc = tag
        .and_then(|t| t.artist())
        .map(|a| a.to_lowercase())
        .unwrap_or_default();
    let genre_lc = tag
        .and_then(|t| t.genre())
        .map(|g| g.to_lowercase())
        .unwrap_or_default();

    if !f.include_artists.is_empty()
        && !f.include_artists.iter().any(|a| a.to_lowercase() == artist_lc)
    {
        return false;
    }
    if f.exclude_artists.iter().any(|a| a.to_lowercase() == artist_lc) {
        return false;
    }
    if !f.include_genres.is_empty()
        && !f.include_genres.iter().any(|g| g.to_lowercase() == genre_lc)
    {
        return false;
    }
    if f.exclude_genres.iter().any(|g| g.to_lowercase() == genre_lc) {
        return false;
    }

    true
}
