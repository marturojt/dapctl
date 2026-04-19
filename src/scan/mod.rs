//! Detect removable drives and apply heuristics to identify DAPs.

pub mod heuristic;
pub mod removable;

use camino::Utf8PathBuf;

/// A removable mount candidate returned by platform-specific scanners.
#[derive(Debug, Clone)]
pub struct Mount {
    pub mount_point: Utf8PathBuf,
    pub label: Option<String>,
    pub filesystem: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

/// A mount that has been matched against a DAP profile id.
#[derive(Debug, Clone)]
pub struct IdentifiedDap {
    pub mount: Mount,
    pub dap_id: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy)]
pub enum Confidence {
    Exact,
    Heuristic,
    Fallback,
}
