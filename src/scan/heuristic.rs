//! Given a `Mount`, try to identify which DAP profile matches it.
//!
//! Signals (roughly, in decreasing strength):
//! - Volume label (`FIIO M21`, `HIBY_R6`, `AK_SR35`).
//! - Presence of firmware-specific marker files (`.database_uuid`, etc.).
//! - Root layout (`/Music`, `/MUSIC`, `/Artists`).
//! - Filesystem + capacity sanity check.

use super::{IdentifiedDap, Mount};

pub fn identify(_mount: &Mount) -> Option<IdentifiedDap> {
    None
}
