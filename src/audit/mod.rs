pub mod report;
pub use report::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const AUDIO_EXTS: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "aiff", "aif", "dsf", "dff", "wv", "ape",
];

const COVER_NAMES: &[&str] = &[
    "folder.jpg",
    "folder.jpeg",
    "cover.jpg",
    "cover.jpeg",
    "front.jpg",
    "front.jpeg",
    "album.jpg",
    "album.jpeg",
];

fn is_audio_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    AUDIO_EXTS.contains(&lower.as_str())
}

struct TrackMeta {
    ext: String,
    title_ok: bool,
    artist_ok: bool,
    album_ok: bool,
    track_num: Option<u32>,
    year_ok: bool,
    has_embedded_cover: bool,
}

fn read_meta(path: &Path) -> TrackMeta {
    use lofty::prelude::{Accessor, TaggedFileExt};

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let tagged = lofty::read_from_path(path).ok();

    let (title_ok, artist_ok, album_ok, track_num, year_ok, has_embedded_cover) =
        if let Some(ref t) = tagged {
            let tag = t.primary_tag().or_else(|| t.first_tag());
            if let Some(tag) = tag {
                let title_ok = tag.title().is_some_and(|s| !s.is_empty());
                let artist_ok = tag.artist().is_some_and(|s| !s.is_empty());
                let album_ok = tag.album().is_some_and(|s| !s.is_empty());
                let track_num = tag.track();
                let year_ok = tag.year().is_some();
                let has_embedded_cover = !tag.pictures().is_empty();
                (
                    title_ok,
                    artist_ok,
                    album_ok,
                    track_num,
                    year_ok,
                    has_embedded_cover,
                )
            } else {
                (false, false, false, None, false, false)
            }
        } else {
            (false, false, false, None, false, false)
        };

    TrackMeta {
        ext,
        title_ok,
        artist_ok,
        album_ok,
        track_num,
        year_ok,
        has_embedded_cover,
    }
}

fn folder_has_cover(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_lc = name.to_string_lossy().to_lowercase();
        if COVER_NAMES.contains(&name_lc.as_str()) {
            return true;
        }
    }
    false
}

fn album_display(dir: &Path, _tracks: &[TrackMeta], library: &Path) -> String {
    // Try to build "Artist / Album" from tags; fall back to relative path.
    use lofty::prelude::{Accessor, TaggedFileExt};
    let first_path = dir.read_dir().ok().and_then(|mut d| {
        d.find_map(|e| {
            let e = e.ok()?;
            let p = e.path();
            let ext = p.extension()?.to_str()?.to_lowercase();
            if is_audio_ext(&ext) {
                Some(p)
            } else {
                None
            }
        })
    });

    if let Some(path) = first_path {
        if let Ok(tagged) = lofty::read_from_path(&path) {
            if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
                let artist = tag.artist().map(|a| a.into_owned());
                let album = tag.album().map(|a| a.into_owned());
                if let (Some(ar), Some(al)) = (artist, album) {
                    if !ar.is_empty() && !al.is_empty() {
                        return format!("{ar} / {al}");
                    }
                }
            }
        }
    }

    // Fallback: path relative to library root.
    dir.strip_prefix(library)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/")
}

fn analyze_album(dir: &Path, tracks: &[TrackMeta]) -> Vec<AlbumIssue> {
    let mut issues: Vec<AlbumIssue> = Vec::new();

    // ── Missing tags ─────────────────────────────────────────────────────────
    let missing_title = tracks.iter().filter(|t| !t.title_ok).count();
    let missing_artist = tracks.iter().filter(|t| !t.artist_ok).count();
    let missing_album = tracks.iter().filter(|t| !t.album_ok).count();
    let missing_tracknum = tracks.iter().filter(|t| t.track_num.is_none()).count();
    let missing_year = tracks.iter().filter(|t| !t.year_ok).count();

    for (field, affected) in [
        ("title", missing_title),
        ("artist", missing_artist),
        ("album", missing_album),
        ("track_num", missing_tracknum),
        ("year", missing_year),
    ] {
        if affected > 0 {
            let issue = Issue::MissingTag {
                field: field.to_owned(),
                affected,
            };
            let severity = issue.severity();
            issues.push(AlbumIssue { severity, issue });
        }
    }

    // ── Cover art ────────────────────────────────────────────────────────────
    let has_embedded = tracks.iter().any(|t| t.has_embedded_cover);
    if !has_embedded && !folder_has_cover(dir) {
        issues.push(AlbumIssue {
            severity: Severity::High,
            issue: Issue::NoCover,
        });
    }

    // ── Format mix ───────────────────────────────────────────────────────────
    let mut exts: Vec<String> = tracks.iter().map(|t| t.ext.to_uppercase()).collect();
    exts.sort();
    exts.dedup();
    if exts.len() > 1 {
        issues.push(AlbumIssue {
            severity: Severity::Medium,
            issue: Issue::FormatMix { formats: exts },
        });
    }

    // ── Track number gaps ────────────────────────────────────────────────────
    let mut nums: Vec<u32> = tracks.iter().filter_map(|t| t.track_num).collect();
    if !nums.is_empty() && nums.len() == tracks.len() {
        // Only check gaps when every track has a number.
        nums.sort();
        nums.dedup();
        let max = *nums.last().unwrap();
        let missing: Vec<u32> = (1..=max).filter(|n| !nums.contains(n)).collect();
        if !missing.is_empty() {
            issues.push(AlbumIssue {
                severity: Severity::Medium,
                issue: Issue::TrackGap { missing },
            });
        }
    }

    // Sort issues within album: high first.
    issues.sort_by_key(|i| i.severity);
    issues
}

/// Walk `library`, inspect every audio file, and return a report of albums
/// with tag or art problems. Read-only — never writes anything.
pub fn scan(library: &Path) -> anyhow::Result<AuditReport> {
    // Group track metadata by album folder.
    let mut by_dir: BTreeMap<PathBuf, Vec<TrackMeta>> = BTreeMap::new();

    for entry in walkdir::WalkDir::new(library)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !is_audio_ext(ext) {
            continue;
        }
        let dir = path.parent().unwrap_or(library).to_path_buf();
        by_dir.entry(dir).or_default().push(read_meta(path));
    }

    let albums_scanned = by_dir.len();
    let tracks_scanned: usize = by_dir.values().map(|v| v.len()).sum();

    let mut album_reports: Vec<AlbumReport> = Vec::new();

    for (dir, tracks) in &by_dir {
        let issues = analyze_album(dir, tracks);
        if issues.is_empty() {
            continue;
        }
        let display = album_display(dir, tracks, library);
        album_reports.push(AlbumReport {
            path: dir.clone(),
            display,
            track_count: tracks.len(),
            issues,
        });
    }

    // Sort albums: most severe first, then alphabetically.
    album_reports.sort_by(|a, b| {
        let sa = a.max_severity().unwrap_or(Severity::Low);
        let sb = b.max_severity().unwrap_or(Severity::Low);
        sa.cmp(&sb).then(a.display.cmp(&b.display))
    });

    let albums_with_issues = album_reports.len();
    let issues_total: usize = album_reports.iter().map(|r| r.issues.len()).sum();
    let high = album_reports
        .iter()
        .flat_map(|r| &r.issues)
        .filter(|i| i.severity == Severity::High)
        .count();
    let medium = album_reports
        .iter()
        .flat_map(|r| &r.issues)
        .filter(|i| i.severity == Severity::Medium)
        .count();
    let low = album_reports
        .iter()
        .flat_map(|r| &r.issues)
        .filter(|i| i.severity == Severity::Low)
        .count();

    Ok(AuditReport {
        library: library.to_path_buf(),
        albums_scanned,
        tracks_scanned,
        albums_with_issues,
        issues_total,
        high,
        medium,
        low,
        albums: album_reports,
    })
}
