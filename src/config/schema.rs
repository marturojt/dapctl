use serde::{Deserialize, Serialize};

/// Top-level sync profile TOML. See `examples/sync-fiio-m21-flac.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProfile {
    pub schema_version: u32,
    pub profile: ProfileHeader,
    #[serde(default)]
    pub filters: Filters,
    #[serde(default)]
    pub transfer: Transfer,
    #[serde(default)]
    pub transcode: Transcode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHeader {
    pub name: String,
    pub source: String,
    pub destination: String,
    pub dap_profile: String,
    pub mode: Mode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Additive,
    Mirror,
    Selective,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filters {
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,

    // ── Tag-based filters (all optional; require lofty to read audio headers) ──
    /// Include only files whose artist tag matches one of these (case-insensitive).
    /// Empty = accept all artists.
    #[serde(default)]
    pub include_artists: Vec<String>,
    /// Exclude files whose artist tag matches any of these (case-insensitive).
    #[serde(default)]
    pub exclude_artists: Vec<String>,
    /// Include only files whose genre tag matches one of these (case-insensitive).
    #[serde(default)]
    pub include_genres: Vec<String>,
    /// Exclude files whose genre tag matches any of these (case-insensitive).
    #[serde(default)]
    pub exclude_genres: Vec<String>,
    /// Skip files with sample rate strictly below this value (Hz).
    #[serde(default)]
    pub min_sample_rate_hz: Option<u32>,
    /// Skip files with sample rate strictly above this value (Hz).
    #[serde(default)]
    pub max_sample_rate_hz: Option<u32>,
    /// Skip files with bit depth strictly below this value.
    #[serde(default)]
    pub min_bit_depth: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    #[serde(default = "default_verify")]
    pub verify: Verify,
    #[serde(default = "default_true")]
    pub dry_run_default: bool,
    #[serde(default = "default_parallelism")]
    pub parallelism: usize,
}

impl Default for Transfer {
    fn default() -> Self {
        Self {
            verify: default_verify(),
            dry_run_default: true,
            parallelism: default_parallelism(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verify {
    None,
    SizeMtime,
    Checksum,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transcode {
    #[serde(default)]
    pub enabled: bool,
}

fn default_verify() -> Verify {
    Verify::SizeMtime
}
fn default_true() -> bool {
    true
}
fn default_parallelism() -> usize {
    4
}
