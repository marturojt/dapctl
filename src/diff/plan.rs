use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Classification of a single file after comparing source and destination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Present in source, absent in destination. Will be copied.
    New,
    /// Present in both, but source is newer/different. Will be overwritten.
    Modified,
    /// Present in destination, absent in source. Deleted in `mirror` mode.
    Orphan,
    /// Present in both, identical. No action.
    Same,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub kind: EntryKind,
    pub path: Utf8PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub entries: Vec<Entry>,
}

impl Plan {
    pub fn total_bytes(&self, kind: EntryKind) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.size_bytes)
            .sum()
    }
}
