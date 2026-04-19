//! dapctl — TUI/CLI sync for HiFi Digital Audio Players.
//!
//! Crate layout mirrors `docs/ARCHITECTURE.md`. Every module exposes the
//! minimum surface needed by `cli` and `tui`; the core logic (config, dap,
//! scan, diff, transfer, log) is stack-agnostic and does not depend on the
//! presentation layer.

pub mod cli;
pub mod config;
pub mod dap;
pub mod diff;
pub mod logging;
pub mod scan;
pub mod transfer;
pub mod tui;
