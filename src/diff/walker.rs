//! Parallel directory walk that yields `(relative_path, size, mtime)` tuples,
//! applying include/exclude globs from the sync profile merged with the
//! DAP profile exclusions.

use camino::Utf8PathBuf;

#[derive(Debug, Clone)]
pub struct Entry {
    pub rel: Utf8PathBuf,
    pub size: u64,
    pub mtime_ns: i128,
}

pub fn walk(_root: &camino::Utf8Path) -> anyhow::Result<Vec<Entry>> {
    Ok(Vec::new())
}
