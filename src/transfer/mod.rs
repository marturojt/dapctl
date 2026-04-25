//! Execute a `Plan`: copy/delete with temp+rename, emit progress events,
//! and maintain a per-run manifest so interrupted syncs can resume.

pub mod executor;
pub mod manifest;
pub mod verify;

pub use executor::{Options, Stats, SyncMode, execute, repair_dest_mtimes};

use serde::{Deserialize, Serialize};

/// Events consumed by the JSONL manifest (written per-run for auditing).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    XferStart { path: String, bytes: u64 },
    XferDone { path: String, bytes: u64 },
    XferFail { path: String, err: String },
    VerifyOk { path: String },
    VerifyFail { path: String, err: String },
    Finish { ok: bool },
}

/// Real-time progress events sent to the TUI over an mpsc channel.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// A new file is about to be copied.
    FileStart { path: String, bytes: u64 },
    /// A chunk was written (incremental bytes).
    FileProgress { bytes: u64 },
    /// File copied and verified successfully.
    FileDone { path: String, bytes: u64 },
    /// File copy or verify failed.
    FileFail { path: String, err: String },
    /// An orphan file was deleted.
    DeleteDone { path: String },
    /// All files processed — final stats.
    Finish { stats: Stats },
}
