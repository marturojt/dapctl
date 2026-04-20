use super::{Confidence, IdentifiedDap, Mount};

/// Try to identify which DAP profile matches a given mount.
///
/// Strategy (highest confidence first):
/// 1. Exact label match against a known table.
/// 2. Partial label match (vendor or model substring).
/// 3. Firmware marker files in the root.
/// 4. Generic fallback when a Music root folder is present.
pub fn identify(mount: &Mount) -> Option<IdentifiedDap> {
    let label_upper = mount
        .label
        .as_deref()
        .map(|l| l.to_uppercase())
        .unwrap_or_default();

    // --- Exact label matches ---
    if matches!(
        label_upper.as_str(),
        "FIIO M21" | "FIIO_M21" | "M21"
    ) {
        return Some(hit(mount, "fiio-m21", Confidence::Exact));
    }
    if matches!(label_upper.as_str(), "AK SR35" | "AK_SR35" | "SR35") {
        return Some(hit(mount, "ak-sr35", Confidence::Exact));
    }
    if matches!(
        label_upper.as_str(),
        "HIBY R6" | "HIBY_R6" | "R6" | "HIBY R6 III"
    ) {
        return Some(hit(mount, "hiby-r6", Confidence::Exact));
    }

    // --- Partial label heuristics ---
    if label_upper.contains("FIIO") {
        return Some(hit(mount, "fiio-m21", Confidence::Heuristic));
    }
    if label_upper.contains("ASTELL") || label_upper.contains("A&K") {
        return Some(hit(mount, "ak-sr35", Confidence::Heuristic));
    }
    if label_upper.contains("HIBY") {
        return Some(hit(mount, "hiby-r6", Confidence::Heuristic));
    }

    // --- Firmware marker files ---
    let root = &mount.mount_point;
    // FiiO devices write `.database_uuid` and a `.thumbnails/` dir
    if root.join(".database_uuid").exists() {
        return Some(hit(mount, "fiio-m21", Confidence::Heuristic));
    }

    // --- Generic fallback: any removable drive with a Music folder ---
    if root.join("Music").exists() || root.join("MUSIC").exists() {
        return Some(hit(mount, "generic", Confidence::Fallback));
    }

    None
}

fn hit(mount: &Mount, dap_id: &str, confidence: Confidence) -> IdentifiedDap {
    IdentifiedDap {
        mount: mount.clone(),
        dap_id: dap_id.to_owned(),
        confidence,
    }
}
