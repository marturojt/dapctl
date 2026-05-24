pub mod compare;
pub mod plan;
pub mod walker;

pub use plan::{Entry, EntryKind, PathWarning, PathWarningKind, Plan};

use camino::Utf8Path;

use crate::config::ResolvedProfile;

/// Run a full diff for a resolved profile.
///
/// Returns the `Plan` and the two entry lists so callers can display
/// additional details or hand them to `transfer::executor`.
pub struct DiffResult {
    pub plan: Plan,
    pub src_count: usize,
    pub dst_count: usize,
}

pub fn diff(
    profile: &ResolvedProfile,
    source: &Utf8Path,
    destination: &Utf8Path,
) -> anyhow::Result<DiffResult> {
    tracing::info!(
        event = "scan_start",
        src = %source,
        dst = %destination,
    );

    let exclude = profile.build_exclude_set()?;
    let include = profile.build_include_set()?;
    let compute_hashes = matches!(
        profile.sync.transfer.verify,
        crate::config::Verify::Checksum
    );
    let transcode_rules = if profile.sync.transcode.enabled {
        profile.sync.transcode.rules.as_slice()
    } else {
        &[]
    };

    // Source: local filesystem or SSH remote.
    let src_entries = if crate::ssh::SshUri::is_ssh(source.as_str()) {
        let uri = crate::ssh::SshUri::parse(source.as_str())?;
        let session = crate::ssh::SshSession::connect(&uri)?;
        // Tag filters and blake3 hashes are not available for remote sources.
        session.walk(&exclude, include.as_ref())?
    } else {
        walker::walk(
            source,
            &exclude,
            include.as_ref(),
            compute_hashes,
            &profile.sync.filters,
            transcode_rules,
        )?
    };

    // Destination is always local and walked WITHOUT transcode projection.
    let dst_entries = walker::walk(
        destination,
        &exclude,
        None,
        compute_hashes,
        &profile.sync.filters,
        &[],
    )?;

    tracing::info!(
        event = "scan_done",
        src = src_entries.len(),
        dst = dst_entries.len(),
    );

    let mut plan = compare::compare(&src_entries, &dst_entries, profile.sync.transfer.verify);
    plan.warnings = check_path_limits(
        &plan.entries,
        &profile.dap.filesystem,
        &profile.dap.layout.music_root,
    );

    tracing::info!(
        event = "plan_ready",
        new = plan.count(EntryKind::New),
        modified = plan.count(EntryKind::Modified),
        orphan = plan.count(EntryKind::Orphan),
        same = plan.count(EntryKind::Same),
        transfer_bytes = plan.transfer_bytes(),
        path_warnings = plan.warnings.len(),
    );

    Ok(DiffResult {
        src_count: src_entries.len(),
        dst_count: dst_entries.len(),
        plan,
    })
}

/// Check every New/Modified entry against the DAP filesystem limits.
/// Returns one warning per entry (filename-too-long takes precedence over path-too-long).
pub fn check_path_limits(
    entries: &[Entry],
    filesystem: &crate::dap::Filesystem,
    music_root: &str,
) -> Vec<PathWarning> {
    let mut warnings = Vec::new();
    for entry in entries {
        if !matches!(entry.kind, EntryKind::New | EntryKind::Modified) {
            continue;
        }
        let mut filename_warned = false;
        for component in entry.path.components() {
            let s = component.as_str();
            if s.len() > filesystem.max_filename_bytes as usize {
                warnings.push(PathWarning {
                    path: entry.path.clone(),
                    kind: PathWarningKind::FilenameTooLong,
                    length_bytes: s.len(),
                    limit_bytes: filesystem.max_filename_bytes,
                });
                filename_warned = true;
                break;
            }
        }
        if !filename_warned {
            // +1 for the '/' separator between music_root and relative path
            let full_len = music_root.len() + 1 + entry.path.as_str().len();
            if full_len > filesystem.max_path_bytes as usize {
                warnings.push(PathWarning {
                    path: entry.path.clone(),
                    kind: PathWarningKind::PathTooLong,
                    length_bytes: full_len,
                    limit_bytes: filesystem.max_path_bytes,
                });
            }
        }
    }
    warnings
}
