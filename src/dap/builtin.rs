//! Builtin DAP profiles embedded via `include_str!`.

/// Canonical profile for FiiO M21 (the author's device, ground-truth source).
pub const FIIO_M21: &str = include_str!("../../profiles/fiio-m21.toml");

/// FiiO M11 Plus (Android-based, DSD512, exFAT).
pub const FIIO_M11: &str = include_str!("../../profiles/fiio-m11.toml");

/// Astell&Kern SR35.
pub const AK_SR35: &str = include_str!("../../profiles/ak-sr35.toml");

/// HiBy R6 Pro III.
pub const HIBY_R6: &str = include_str!("../../profiles/hiby-r6.toml");

/// Shanling M3 Ultra.
pub const SHANLING_M3ULTRA: &str = include_str!("../../profiles/shanling-m3ultra.toml");

/// iBasso DX320.
pub const IBASSO_DX320: &str = include_str!("../../profiles/ibasso-dx320.toml");

/// Conservative fallback for unknown DAPs.
pub const GENERIC: &str = include_str!("../../profiles/generic.toml");

/// All builtin profiles as `(id, toml)`.
pub const ALL: &[(&str, &str)] = &[
    ("ak-sr35", AK_SR35),
    ("fiio-m11", FIIO_M11),
    ("fiio-m21", FIIO_M21),
    ("generic", GENERIC),
    ("hiby-r6", HIBY_R6),
    ("ibasso-dx320", IBASSO_DX320),
    ("shanling-m3ultra", SHANLING_M3ULTRA),
];
