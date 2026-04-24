use super::{Confidence, IdentifiedDap, Mount};

/// Try to identify which DAP profile matches a given mount.
///
/// **Prerequisite:** `mount.label` must contain the actual volume label
/// (not the device node). On Windows this requires `GetVolumeInformationW`;
/// see `scan::removable`.
///
/// Strategy (highest confidence first):
/// 1. Exact label match against a known table.
/// 2. Vendor/model substring in label.
/// 3. Firmware marker files in the root.
/// 4. Generic fallback: exFAT/FAT32 removable with a Music folder.
pub fn identify(mount: &Mount) -> Option<IdentifiedDap> {
    let label_up = mount
        .label
        .as_deref()
        .map(|l| l.to_uppercase())
        .unwrap_or_default();

    // ── FiiO exact labels ────────────────────────────────────────────────
    if matches!(label_up.as_str(), "FIIO M21" | "FIIO_M21" | "M21") {
        return Some(hit(mount, "fiio-m21", Confidence::Exact));
    }
    // M11 family (M11, M11 Plus, M11S, M11 Pro)
    if label_up == "FIIO M11"
        || label_up.starts_with("M11")
        || label_up.contains("M11 PLUS")
        || label_up.contains("M11PLUS")
        || label_up.contains("M11_PLUS")
        || label_up.contains("M11 PRO")
        || label_up.contains("M11S")
    {
        // No dedicated profile yet — map to generic until a contributor adds one
        return Some(hit(mount, "generic", Confidence::Heuristic));
    }

    // ── Astell & Kern ────────────────────────────────────────────────────
    if matches!(label_up.as_str(), "AK SR35" | "AK_SR35" | "SR35") {
        return Some(hit(mount, "ak-sr35", Confidence::Exact));
    }

    // ── HiBy ────────────────────────────────────────────────────────────
    if matches!(
        label_up.as_str(),
        "HIBY R6" | "HIBY_R6" | "R6" | "HIBY R6 III"
    ) {
        return Some(hit(mount, "hiby-r6", Confidence::Exact));
    }

    // ── Vendor substring heuristics ──────────────────────────────────────
    if label_up.contains("FIIO") {
        return Some(hit(mount, "fiio-m21", Confidence::Heuristic));
    }
    if label_up.contains("ASTELL") || label_up.contains("A&K") || label_up.contains("AK") {
        return Some(hit(mount, "ak-sr35", Confidence::Heuristic));
    }
    if label_up.contains("HIBY") {
        return Some(hit(mount, "hiby-r6", Confidence::Heuristic));
    }
    if label_up.contains("SHANLING") || label_up.contains("IBASSO") || label_up.contains("CAYIN") {
        return Some(hit(mount, "generic", Confidence::Heuristic));
    }

    // ── Firmware marker files ────────────────────────────────────────────
    let root = &mount.mount_point;
    // FiiO Android firmware creates .database_uuid at the SD card root
    if root.join(".database_uuid").exists() {
        return Some(hit(mount, "fiio-m21", Confidence::Heuristic));
    }
    // HiBy leaves a HiByMusic folder
    if root.join("HiByMusic").exists() {
        return Some(hit(mount, "hiby-r6", Confidence::Heuristic));
    }
    // .thumbnails/ at SD root is a common Android DAP artifact (HiBy, FiiO, etc.)
    if root.join(".thumbnails").exists() {
        return Some(hit(mount, "generic", Confidence::Heuristic));
    }

    // ── Generic fallback ─────────────────────────────────────────────────
    // Any exFAT/FAT32 removable with a Music folder is likely a DAP microSD.
    let is_audio_fs = mount
        .filesystem
        .as_deref()
        .map(|fs| matches!(fs, "EXFAT" | "FAT32" | "FAT" | "VFAT"))
        .unwrap_or(false);

    if is_audio_fs && (root.join("Music").exists() || root.join("MUSIC").exists()) {
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
