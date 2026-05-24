use std::path::{Path, PathBuf};

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::error::ConfigError;

pub mod schema;

pub use schema::{
    Filters, Mode, Selective, SyncProfile, Transcode, TranscodeRule, Transfer, Verify,
};

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Parse and validate a sync profile from a TOML file.
pub fn load(path: &Path) -> anyhow::Result<SyncProfile> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Read {
        path: path.to_owned(),
        source: e,
    })?;
    let profile: SyncProfile = toml::from_str(&content).map_err(|e| ConfigError::Parse {
        path: path.to_owned(),
        reason: e.to_string(),
    })?;
    validate(&profile)?;
    Ok(profile)
}

/// Find a sync profile by name, searching the config profiles directory.
/// Also accepts an explicit file path (absolute or relative).
pub fn find(name_or_path: &str) -> anyhow::Result<SyncProfile> {
    // 1. Exact path?
    let as_path = Path::new(name_or_path);
    if as_path.exists() {
        return load(as_path);
    }

    // 2. In config dir
    let dir = profiles_dir()?;
    let candidate = dir.join(format!("{name_or_path}.toml"));
    if candidate.exists() {
        return load(&candidate);
    }

    Err(ConfigError::NotFound {
        name: name_or_path.to_owned(),
    }
    .into())
}

/// List all sync profiles discovered in the config profiles directory.
/// Returns `(name, path)` pairs, sorted by name.
pub fn discover() -> anyhow::Result<Vec<(String, PathBuf)>> {
    let dir = profiles_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut profiles = Vec::new();
    for entry in
        std::fs::read_dir(&dir).with_context(|| format!("cannot read config dir {dir:?}"))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                profiles.push((stem.to_owned(), path));
            }
        }
    }
    profiles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(profiles)
}

/// `$XDG_CONFIG_HOME/dapctl/profiles/` (or platform equivalent).
pub fn profiles_dir() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")
        .ok_or_else(|| anyhow::anyhow!("cannot determine config directory"))?;
    Ok(dirs.config_dir().join("profiles"))
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate(p: &SyncProfile) -> Result<(), ConfigError> {
    if p.schema_version != 1 {
        return Err(ConfigError::UnsupportedVersion {
            got: p.schema_version,
            expected: 1,
        });
    }
    if p.profile.name.trim().is_empty() {
        return Err(ConfigError::MissingField {
            section: "profile",
            field: "name",
        });
    }
    if p.profile.source.trim().is_empty() {
        return Err(ConfigError::MissingField {
            section: "profile",
            field: "source",
        });
    }
    if p.profile.destination.trim().is_empty() {
        return Err(ConfigError::MissingField {
            section: "profile",
            field: "destination",
        });
    }
    if p.profile.dap_profile.trim().is_empty() {
        return Err(ConfigError::MissingField {
            section: "profile",
            field: "dap_profile",
        });
    }
    for g in p
        .filters
        .include_globs
        .iter()
        .chain(&p.filters.exclude_globs)
    {
        Glob::new(g).map_err(|e| ConfigError::InvalidGlob {
            pattern: g.clone(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Resolved profile (sync + DAP merged)
// ---------------------------------------------------------------------------

/// A sync profile and its associated DAP profile, fully resolved.
/// This is the canonical input to `scan`, `diff`, and `transfer`.
#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub sync: SyncProfile,
    pub dap: crate::dap::DapProfile,
}

impl ResolvedProfile {
    /// Merge exclusion globs: DAP profile globs first, then sync profile globs.
    pub fn all_exclude_globs(&self) -> impl Iterator<Item = &str> {
        self.dap
            .exclude
            .globs
            .iter()
            .chain(self.sync.filters.exclude_globs.iter())
            .map(String::as_str)
    }

    pub fn build_exclude_set(&self) -> anyhow::Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();
        for pattern in self.all_exclude_globs() {
            builder.add(Glob::new(pattern)?);
        }
        Ok(builder.build()?)
    }

    /// Include globs from the sync profile. Empty = accept all.
    pub fn build_include_set(&self) -> anyhow::Result<Option<GlobSet>> {
        let globs = &self.sync.filters.include_globs;
        if globs.is_empty() {
            return Ok(None);
        }
        let mut builder = GlobSetBuilder::new();
        for pattern in globs {
            builder.add(Glob::new(pattern)?);
        }
        Ok(Some(builder.build()?))
    }
}

/// Rewrite only the `[selective].include_paths` key in `path`, preserving all
/// other content (comments, ordering, whitespace) via toml_edit.
pub fn save_selective_paths(path: &std::path::Path, paths: &[String]) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path).with_context(|| format!("cannot read {path:?}"))?;
    let mut doc: toml_edit::DocumentMut = content
        .parse()
        .with_context(|| format!("cannot parse {path:?} as TOML"))?;

    let sel = doc.entry("selective").or_insert(toml_edit::table());
    let mut arr = toml_edit::Array::new();
    for p in paths {
        arr.push(p.as_str());
    }
    sel["include_paths"] = toml_edit::value(arr);

    std::fs::write(path, doc.to_string()).with_context(|| format!("cannot write {path:?}"))
}

/// Load a sync profile by name and resolve its DAP profile.
pub fn resolve(name_or_path: &str) -> anyhow::Result<ResolvedProfile> {
    let sync = find(name_or_path)?;
    let dap = crate::dap::load(&sync.profile.dap_profile)?;
    Ok(ResolvedProfile { sync, dap })
}
