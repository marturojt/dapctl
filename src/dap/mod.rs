use anyhow::Context;

pub mod builtin;
pub mod schema;

pub use schema::{Codecs, DapHeader, DapProfile, Exclude, Filesystem, Layout, Quirks};

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load a DAP profile by id.
/// Checks `$XDG_CONFIG_HOME/dapctl/profiles/<id>.toml` first (user override),
/// then falls back to the builtin catalogue embedded at compile time.
pub fn load(id: &str) -> anyhow::Result<DapProfile> {
    if let Some(p) = load_user_override(id)? {
        tracing::debug!(dap_id = id, source = "user-override", "loaded DAP profile");
        return Ok(p);
    }
    let p = load_builtin(id)?;
    tracing::debug!(dap_id = id, source = "builtin", "loaded DAP profile");
    Ok(p)
}

/// List ids of all known DAP profiles: builtins first, then user overrides
/// (overrides that shadow a builtin appear only once).
pub fn list() -> anyhow::Result<Vec<String>> {
    let mut ids: Vec<String> = builtin::ALL.iter().map(|(id, _)| id.to_string()).collect();

    let Some(dirs) = directories::ProjectDirs::from("", "", "dapctl") else {
        return Ok(ids);
    };
    let override_dir = dirs.config_dir().join("profiles");
    if override_dir.exists() {
        for entry in std::fs::read_dir(&override_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let stem = stem.to_owned();
                    if !ids.contains(&stem) {
                        ids.push(stem);
                    }
                }
            }
        }
    }
    ids.sort();
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn load_builtin(id: &str) -> anyhow::Result<DapProfile> {
    let toml_str = builtin::ALL
        .iter()
        .find(|(bid, _)| *bid == id)
        .map(|(_, s)| *s)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown DAP profile id {id:?}; \
                 run `dapctl profile list` to see available profiles"
            )
        })?;

    toml::from_str(toml_str)
        .with_context(|| format!("failed to parse builtin DAP profile {id:?}"))
}

fn load_user_override(id: &str) -> anyhow::Result<Option<DapProfile>> {
    let Some(dirs) = directories::ProjectDirs::from("", "", "dapctl") else {
        return Ok(None);
    };
    let path = dirs.config_dir().join("profiles").join(format!("{id}.toml"));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read DAP profile override {path:?}"))?;
    let profile: DapProfile = toml::from_str(&content)
        .with_context(|| format!("invalid DAP profile override {path:?}"))?;
    Ok(Some(profile))
}
