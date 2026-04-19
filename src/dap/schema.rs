use serde::{Deserialize, Serialize};

/// Top-level DAP profile TOML. See `docs/DAP_PROFILE_SPEC.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DapProfile {
    pub schema_version: u32,
    pub dap: DapHeader,
    pub filesystem: Filesystem,
    pub codecs: Codecs,
    pub layout: Layout,
    #[serde(default)]
    pub exclude: Exclude,
    #[serde(default)]
    pub quirks: Quirks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapHeader {
    pub id: String,
    pub name: String,
    pub vendor: String,
    #[serde(default)]
    pub firmware_min: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filesystem {
    pub preferred: String,
    pub supported: Vec<String>,
    pub max_filename_bytes: u32,
    pub max_path_bytes: u32,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Codecs {
    #[serde(default)]
    pub lossless: Vec<String>,
    #[serde(default)]
    pub lossy: Vec<String>,
    pub max_sample_rate_hz: u32,
    pub max_bit_depth: u32,
    #[serde(default)]
    pub dsd: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub music_root: String,
    #[serde(default)]
    pub prefers_artist_album_tree: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Exclude {
    #[serde(default)]
    pub globs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Quirks {
    #[serde(default)]
    pub warn_on_embedded_art_mb: Option<u32>,
    #[serde(default)]
    pub normalize_unicode: Option<String>,
}
