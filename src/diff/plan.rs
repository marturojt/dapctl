use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Present in source, absent in destination → will be copied.
    New,
    /// Present in both, but different → will be overwritten.
    Modified,
    /// Present in destination only → deleted in mirror mode.
    Orphan,
    /// Identical → no action.
    Same,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub kind: EntryKind,
    pub path: Utf8PathBuf,
    pub size_bytes: u64,
    /// When `Some`, the file must be transcoded from the given source extension.
    /// E.g. `Some("dsf")` means `path` ends in `.flac` but the source file is `.dsf`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcode_from: Option<String>,
}

/// Reason a path may cause problems on the target DAP filesystem.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PathWarningKind {
    /// A single path component (directory or filename) exceeds `max_filename_bytes`.
    FilenameTooLong,
    /// The full on-device path (music_root + relative) exceeds `max_path_bytes`.
    PathTooLong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathWarning {
    pub path: Utf8PathBuf,
    pub kind: PathWarningKind,
    pub length_bytes: usize,
    pub limit_bytes: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub entries: Vec<Entry>,
    /// Paths that may be rejected or truncated by the DAP firmware.
    /// Only populated when a DAP profile is available (i.e. via `diff::diff()`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<PathWarning>,
}

impl Plan {
    pub fn count(&self, kind: EntryKind) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    pub fn total_bytes(&self, kind: EntryKind) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.size_bytes)
            .sum()
    }

    /// Bytes that will need to be written to the destination.
    pub fn transfer_bytes(&self) -> u64 {
        self.total_bytes(EntryKind::New) + self.total_bytes(EntryKind::Modified)
    }

    /// Rough ETA in seconds assuming `speed_bps` bytes per second.
    pub fn eta_secs(&self, speed_bps: u64) -> u64 {
        if speed_bps == 0 {
            return 0;
        }
        self.transfer_bytes() / speed_bps
    }
}
