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
        Verify::SizeMtime => {
            if same_size_mtime(src, dst) { EntryKind::Same } else { EntryKind::Modified }
        }
        Verify::Checksum => {
            if src.size != dst.size {
                return EntryKind::Modified;
            }
            match (src.hash, dst.hash) {
                (Some(sh), Some(dh)) => {
                    if sh == dh { EntryKind::Same } else { EntryKind::Modified }
                }
                // Hashes absent (compute_hashes was false): fall back to mtime.
                _ => {
                    if same_mtime(src, dst) { EntryKind::Same } else { EntryKind::Modified }
                }
            }
        }
    }
}

fn same_size_mtime(src: &WalkEntry, dst: &WalkEntry) -> bool {
    if src.size != dst.size {
        return false;
    }
    same_mtime(src, dst)
}

fn same_mtime(src: &WalkEntry, dst: &WalkEntry) -> bool {
    (src.mtime_ns - dst.mtime_ns).abs() <= MTIME_TOLERANCE_NS
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    fn entry(path: &str, size: u64, mtime_ns: i128) -> WalkEntry {
        WalkEntry { rel: Utf8PathBuf::from(path), size, mtime_ns, hash: None }
    }

    fn entry_hashed(path: &str, size: u64, data: &[u8]) -> WalkEntry {
        let hash = Some(blake3::hash(data));
        WalkEntry { rel: Utf8PathBuf::from(path), size, mtime_ns: 0, hash }
    }

    const T: i128 = 1_000_000_000_000; // arbitrary base timestamp

    #[test]
    fn empty_both_produces_empty_plan() {
        let plan = compare(&[], &[], Verify::SizeMtime);
        assert!(plan.entries.is_empty());
    }

    #[test]
    fn all_new_when_dst_empty() {
        let src = vec![entry("a.flac", 100, T), entry("b.flac", 200, T)];
        let plan = compare(&src, &[], Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::New), 2);
        assert_eq!(plan.count(EntryKind::Orphan), 0);
    }

    #[test]
    fn all_orphan_when_src_empty() {
        let dst = vec![entry("a.flac", 100, T), entry("b.flac", 200, T)];
        let plan = compare(&[], &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::Orphan), 2);
        assert_eq!(plan.count(EntryKind::New), 0);
    }

    #[test]
    fn identical_entries_are_same() {
        let src = vec![entry("a.flac", 100, T)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::Same), 1);
    }

    #[test]
    fn different_size_is_modified() {
        let src = vec![entry("a.flac", 200, T)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::Modified), 1);
    }

    #[test]
    fn mtime_diff_beyond_tolerance_is_modified() {
        // 3 seconds apart — exceeds the 2 s FAT32 tolerance.
        let src = vec![entry("a.flac", 100, T + 3_000_000_001)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::Modified), 1);
    }

    #[test]
    fn mtime_diff_within_fat32_tolerance_is_same() {
        // 2 seconds apart — within FAT32 granularity, treated as Same.
        let src = vec![entry("a.flac", 100, T + 2_000_000_000)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::Same), 1);
    }

    #[test]
    fn verify_none_always_same() {
        // Even with different sizes, Verify::None never marks Modified.
        let src = vec![entry("a.flac", 999, T + 10_000_000_000)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::None);
        assert_eq!(plan.count(EntryKind::Same), 1);
        assert_eq!(plan.count(EntryKind::Modified), 0);
    }

    #[test]
    fn mixed_plan_counts_correctly() {
        let src = vec![
            entry("album/01.flac", 100, T),      // Same
            entry("album/02.flac", 200, T),      // Modified (size change)
            entry("album/03.flac", 300, T),      // New
        ];
        let dst = vec![
            entry("album/01.flac", 100, T),      // Same
            entry("album/02.flac", 150, T),      // → Modified
            entry("album/04.flac", 400, T),      // Orphan
        ];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        assert_eq!(plan.count(EntryKind::New),      1, "new");
        assert_eq!(plan.count(EntryKind::Modified), 1, "modified");
        assert_eq!(plan.count(EntryKind::Orphan),   1, "orphan");
        assert_eq!(plan.count(EntryKind::Same),     1, "same");
    }

    #[test]
    fn checksum_same_hash_is_same() {
        let data = b"identical content";
        let src = vec![entry_hashed("a.flac", data.len() as u64, data)];
        let dst = vec![entry_hashed("a.flac", data.len() as u64, data)];
        let plan = compare(&src, &dst, Verify::Checksum);
        assert_eq!(plan.count(EntryKind::Same), 1);
        assert_eq!(plan.count(EntryKind::Modified), 0);
    }

    #[test]
    fn checksum_different_hash_same_size_is_modified() {
        let src = vec![entry_hashed("a.flac", 10, b"source data")];
        let dst = vec![entry_hashed("a.flac", 10, b"dest__data")];
        let plan = compare(&src, &dst, Verify::Checksum);
        assert_eq!(plan.count(EntryKind::Modified), 1);
    }

    #[test]
    fn checksum_no_hashes_falls_back_to_mtime() {
        // Without hashes, Verify::Checksum behaves like SizeMtime.
        let src = vec![entry("a.flac", 100, T + 5_000_000_000)];
        let dst = vec![entry("a.flac", 100, T)];
        let plan = compare(&src, &dst, Verify::Checksum);
        assert_eq!(plan.count(EntryKind::Modified), 1);
    }

    #[test]
    fn transfer_bytes_counts_new_and_modified_only() {
        let src = vec![
            entry("new.flac",      500, T),
            entry("modified.flac", 300, T + 5_000_000_000),
            entry("same.flac",     100, T),
        ];
        let dst = vec![
            entry("modified.flac", 300, T),
            entry("orphan.flac",   200, T),
            entry("same.flac",     100, T),
        ];
        let plan = compare(&src, &dst, Verify::SizeMtime);
        // transfer_bytes = new(500) + modified(300)
        assert_eq!(plan.transfer_bytes(), 800);
    }
}
