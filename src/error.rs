//! Typed error taxonomy for dapctl.
//!
//! Every variant carries a user-readable message via its `Display` impl
//! (authored in the `#[error("...")]` attribute).  All enums implement
//! `std::error::Error` and convert to `anyhow::Error` automatically so
//! callers that return `anyhow::Result<T>` can use `?` without changes.
//!
//! Exit-code mapping (see `main.rs`):
//!   2 — user / config error  (ConfigError, DapError, profile-not-found)
//!   3 — environment error    (ScanError)
//!   1 — everything else      (default)

use std::path::PathBuf;

// ── Config ────────────────────────────────────────────────────────────────────

/// Errors arising from loading or validating a sync profile.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required field [{section}] {field}")]
    MissingField {
        section: &'static str,
        field: &'static str,
    },

    #[error(
        "unsupported schema_version {got} (this build supports version {expected})"
    )]
    UnsupportedVersion { got: u32, expected: u32 },

    #[error("invalid glob pattern {pattern:?}: {reason}")]
    InvalidGlob { pattern: String, reason: String },

    #[error("cannot read sync profile {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot write sync profile {path:?}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot parse sync profile {path:?}: {reason}")]
    Parse { path: PathBuf, reason: String },

    #[error("sync profile {name:?} not found; check the profiles dir with `dapctl profile list`")]
    NotFound { name: String },
}

// ── DAP catalogue ─────────────────────────────────────────────────────────────

/// Errors arising from loading a DAP device profile.
#[derive(Debug, thiserror::Error)]
pub enum DapError {
    #[error(
        "unknown DAP profile {id:?}; \
         run `dapctl profile list` to see available profiles"
    )]
    UnknownId { id: String },

    #[error("failed to parse builtin DAP profile {id:?}: {reason}")]
    ParseBuiltin { id: String, reason: String },

    #[error("cannot read DAP profile override {path:?}: {source}")]
    ReadOverride {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid DAP profile override {path:?}: {reason}")]
    InvalidOverride { path: PathBuf, reason: String },
}

// ── Scan / destination resolution ────────────────────────────────────────────

/// Errors arising from device scanning or destination resolution.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(
        "no connected drive identified as DAP {dap_id:?}; \
         run `dapctl scan` to see connected drives"
    )]
    DestinationNotFound { dap_id: String },
}
