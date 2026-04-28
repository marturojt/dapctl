use std::fmt::Write as FmtWrite;

use camino::Utf8Path;

use crate::config::ResolvedProfile;
use crate::diff::walker::{self, Entry};

/// Generate an M3U playlist for a resolved profile.
///
/// Walks the source with all configured filters (glob, tag, transcode projection)
/// to produce the same file list that a sync would copy. Each entry's path is
/// prefixed with the DAP's `layout.music_root` so the playlist is usable when
/// placed on the DAP's storage.
///
/// Returns the M3U content as a `String`.
pub fn generate(
    profile: &ResolvedProfile,
    source: &Utf8Path,
) -> anyhow::Result<String> {
    let exclude = profile.build_exclude_set()?;
    let include = profile.build_include_set()?;
    let transcode_rules = if profile.sync.transcode.enabled {
        profile.sync.transcode.rules.as_slice()
    } else {
        &[]
    };

    let entries: Vec<Entry> =
        walker::walk(source, &exclude, include.as_ref(), false, &profile.sync.filters, transcode_rules)?;

    let music_root = profile.dap.layout.music_root.trim_end_matches('/');
    let mut out = String::from("#EXTM3U\n");

    for e in &entries {
        // Normalise to forward slashes (the M3U will live on the DAP).
        let rel = e.rel.as_str().replace('\\', "/");
        writeln!(out, "{music_root}/{rel}").unwrap();
    }

    Ok(out)
}
