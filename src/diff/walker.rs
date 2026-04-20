use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use globset::GlobSet;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to the walk root, with `/` separators (platform-independent).
    pub rel: Utf8PathBuf,
    pub size: u64,
    /// Modification time as nanoseconds since UNIX epoch. 0 if unavailable.
    pub mtime_ns: i128,
}

/// Walk `root` recursively, applying exclusion and inclusion globs.
///
/// Returns entries sorted by `rel` path for O(n) merge-join in `compare`.
pub fn walk(
    root: &Utf8Path,
    exclude: &GlobSet,
    include: Option<&GlobSet>,
) -> anyhow::Result<Vec<Entry>> {
    if !root.exists() {
        // Destination may not exist yet on first sync — return empty.
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for result in WalkDir::new(root).follow_links(false) {
        let de = result.with_context(|| format!("error walking {root}"))?;

        if !de.file_type().is_file() {
            continue;
        }

        // Build a `/`-separated relative path for cross-platform glob matching.
        let abs = de.path();
        let rel_os = abs
            .strip_prefix(root.as_std_path())
            .with_context(|| format!("{} is not under {root}", abs.display()))?;
        let rel_str = rel_os.to_string_lossy().replace('\\', "/");
        let rel = Utf8PathBuf::from(&rel_str);

        if exclude.is_match(&rel_str) {
            continue;
        }
        if let Some(inc) = include {
            if !inc.is_match(&rel_str) {
                continue;
            }
        }

        let meta = de.metadata()?;
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i128)
            .unwrap_or(0);

        entries.push(Entry {
            rel,
            size: meta.len(),
            mtime_ns,
        });
    }

    entries.sort_unstable_by(|a, b| a.rel.cmp(&b.rel));
    Ok(entries)
}
