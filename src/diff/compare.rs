use std::cmp::Ordering;

use crate::config::Verify;

use super::{
    plan::{Entry as PlanEntry, EntryKind, Plan},
    walker::Entry as WalkEntry,
};

/// FAT32 mtime granularity is 2 seconds. Allow up to 2 s of drift when
/// comparing source (often ext4/APFS, nanosecond precision) vs destination
/// (FAT32/exFAT, 2 s granularity).
const MTIME_TOLERANCE_NS: i128 = 2_000_000_000;

/// Produce a `Plan` from two sorted entry lists.
///
/// Both slices must be sorted by `rel` path (walker guarantees this).
pub fn compare(src: &[WalkEntry], dst: &[WalkEntry], verify: Verify) -> Plan {
    let mut entries = Vec::with_capacity(src.len().max(dst.len()));
    let mut si = 0usize;
    let mut di = 0usize;

    while si < src.len() && di < dst.len() {
        match src[si].rel.cmp(&dst[di].rel) {
            Ordering::Less => {
                entries.push(PlanEntry {
                    kind: EntryKind::New,
                    path: src[si].rel.clone(),
                    size_bytes: src[si].size,
                });
                si += 1;
            }
            Ordering::Greater => {
                entries.push(PlanEntry {
                    kind: EntryKind::Orphan,
                    path: dst[di].rel.clone(),
                    size_bytes: dst[di].size,
                });
                di += 1;
            }
            Ordering::Equal => {
                let kind = classify(&src[si], &dst[di], verify);
                entries.push(PlanEntry {
                    kind,
                    path: src[si].rel.clone(),
                    size_bytes: src[si].size,
                });
                si += 1;
                di += 1;
            }
        }
    }

    // Remaining source entries → New
    for e in &src[si..] {
        entries.push(PlanEntry {
            kind: EntryKind::New,
            path: e.rel.clone(),
            size_bytes: e.size,
        });
    }

    // Remaining destination entries → Orphan
    for e in &dst[di..] {
        entries.push(PlanEntry {
            kind: EntryKind::Orphan,
            path: e.rel.clone(),
            size_bytes: e.size,
        });
    }

    Plan { entries }
}

fn classify(src: &WalkEntry, dst: &WalkEntry, verify: Verify) -> EntryKind {
    match verify {
        Verify::None => EntryKind::Same,
        Verify::SizeMtime | Verify::Checksum => {
            // Checksum falls back to size+mtime until Verify::Checksum
            // with blake3 is implemented in transfer::verify.
            if same_size_mtime(src, dst) {
                EntryKind::Same
            } else {
                EntryKind::Modified
            }
        }
    }
}

fn same_size_mtime(src: &WalkEntry, dst: &WalkEntry) -> bool {
    if src.size != dst.size {
        return false;
    }
    (src.mtime_ns - dst.mtime_ns).abs() <= MTIME_TOLERANCE_NS
}
