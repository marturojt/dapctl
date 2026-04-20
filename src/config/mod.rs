use std::path::{Path, PathBuf};

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};

pub mod schema;

pub use schema::{Filters, Mode, SyncProfile, Transcode, Transfer, Verify};

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Parse and validate a sync profile from a TOML file.
pub fn load(path: &Path) -> anyhow::Result<SyncProfile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read sync profile {path:?}"))?;
    let profile: SyncProfile = toml::from_str(&content)
        .with_context(|| format!("invalid sync profile {path:?}"))?;
    validate(&profile).with_context(|| format!("sync profile {path:?} failed validation"))?;
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

    anyhow::bail!(
        "sync profile {name_or_path:?} not found \
         (tried {candidate:?} and as a literal path)"
    )
}

/// List all sync profiles discovered in the config profiles directory.
/// Returns `(name, path)` pairs, sorted by name.
pub fn discover() -> anyhow::Result<Vec<(String, PathBuf)>> {
    let dir = profiles_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("cannot read config dir {dir:?}"))?
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

fn validate(p: &SyncProfile) -> anyhow::Result<()> {
    if p.schema_version != 1 {
        anyhow::bail!(
            "unsupported schema_version {} (expected 1)",
            p.schema_version
        );
    }
    if p.profile.name.trim().is_empty() {
        anyhow::bail!("[profile] name is required");
    }
    if p.profile.source.trim().is_empty() {
        anyhow::bail!("[profile] source is required");
    }
    if p.profile.destination.trim().is_empty() {
        anyhow::bail!("[profile] destination is required");
    }
    if p.profile.dap_profile.trim().is_empty() {
        anyhow::bail!("[profile] dap_profile is required");
    }
    // Validate globs are parseable
    for g in p
        .filters
        .include_globs
        .iter()
        .chain(&p.filters.exclude_globs)
    {
        Glob::new(g).with_context(|| format!("invalid glob {g:?}"))?;
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

/// Load a sync profile by name and resolve its DAP profile.
pub fn resolve(name_or_path: &str) -> anyhow::Result<ResolvedProfile> {
    let sync = find(name_or_path)?;
    let dap = crate::dap::load(&sync.profile.dap_profile)?;
    Ok(ResolvedProfile { sync, dap })
}
