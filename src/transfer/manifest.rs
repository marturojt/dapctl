//! Per-run manifest for crash/interruption recovery.
//!
//! Format: JSONL written to `$XDG_STATE_HOME/dapctl/runs/<ulid>.jsonl`.
//! One entry per planned file, updated in place by rewriting the line.
//! On resume, any entry not in `Done` state is re-queued.

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    InProgress,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: Utf8PathBuf,
    pub size_bytes: u64,
    pub state: State,
    #[serde(default)]
    pub err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub run_id: String,
    pub profile: String,
    pub started_at: String,
    pub entries: Vec<ManifestEntry>,
}
