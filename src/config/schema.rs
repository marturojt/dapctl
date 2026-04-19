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
