pub mod compare;
pub mod plan;
pub mod walker;

pub use plan::{Entry, EntryKind, Plan};

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
    let compute_hashes = matches!(profile.sync.transfer.verify, crate::config::Verify::Checksum);
    let transcode_rules = if profile.sync.transcode.enabled {
        profile.sync.transcode.rules.as_slice()
    } else {
        &[]
    };

    // Destination is walked WITHOUT transcode projection — it contains real files.
    let src_entries = walker::walk(source, &exclude, include.as_ref(), compute_hashes, &profile.sync.filters, transcode_rules)?;
    let dst_entries = walker::walk(destination, &exclude, None, compute_hashes, &profile.sync.filters, &[])?;

    tracing::info!(
        event = "scan_done",
        src = src_entries.len(),
        dst = dst_entries.len(),
    );

    let plan = compare::compare(&src_entries, &dst_entries, profile.sync.transfer.verify);

    tracing::info!(
        event = "plan_ready",
        new    = plan.count(EntryKind::New),
        modified = plan.count(EntryKind::Modified),
        orphan = plan.count(EntryKind::Orphan),
        same   = plan.count(EntryKind::Same),
        transfer_bytes = plan.transfer_bytes(),
    );

    Ok(DiffResult {
        src_count: src_entries.len(),
        dst_count: dst_entries.len(),
        plan,
    })
}
