//! Builtin DAP profiles embedded via `include_str!`.

/// Canonical profile for FiiO M21 (the author's device, ground-truth source).
pub const FIIO_M21: &str = include_str!("../../profiles/fiio-m21.toml");

/// Conservative fallback for unknown DAPs.
pub const GENERIC: &str = include_str!("../../profiles/generic.toml");

/// All builtin profiles as `(id, toml)`.
pub const ALL: &[(&str, &str)] = &[("fiio-m21", FIIO_M21), ("generic", GENERIC)];
