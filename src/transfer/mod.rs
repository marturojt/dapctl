//! Execute a `Plan`: copy/delete with temp+rename, emit progress events,
//! and maintain a per-run manifest so interrupted syncs can resume.

pub mod executor;
pub mod manifest;
pub mod verify;

pub use executor::{Options, Stats, SyncMode, execute};

use serde::{Deserialize, Serialize};

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
