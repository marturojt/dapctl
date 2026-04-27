//! Integration tests for the diff pipeline: walker + compare + plan.
//!
//! Each test builds real temp directories so we exercise the actual filesystem
//! code paths that run in production.

use std::time::{Duration, UNIX_EPOCH};

use assert_fs::prelude::*;
use assert_fs::TempDir;
use camino::Utf8PathBuf;
use globset::GlobSetBuilder;

use dapctl::diff::EntryKind;
use dapctl::diff::compare::compare;
use dapctl::diff::walker::walk;
use dapctl::config::Verify;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn empty_globset() -> globset::GlobSet {
    GlobSetBuilder::new().build().unwrap()
}

fn write_file(dir: &TempDir, rel: &str, content: &[u8]) {
    dir.child(rel).write_binary(content).unwrap();
}

/// Set the mtime of a file relative to the temp dir.
fn set_mtime(dir: &TempDir, rel: &str, secs_since_epoch: u64) {
    let path = dir.child(rel).path().to_owned();
    let t = UNIX_EPOCH + Duration::from_secs(secs_since_epoch);
    filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(t)).unwrap();
}

fn walk_dir(dir: &TempDir) -> Vec<dapctl::diff::walker::Entry> {
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    walk(&root, &empty_globset(), None, false).unwrap()
}

fn walk_dir_hashed(dir: &TempDir) -> Vec<dapctl::diff::walker::Entry> {
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    walk(&root, &empty_globset(), None, true).unwrap()
}

// ── Walker tests ──────────────────────────────────────────────────────────────

#[test]
fn walker_empty_dir_returns_empty() {
    let dir = TempDir::new().unwrap();
    let entries = walk_dir(&dir);
    assert!(entries.is_empty());
}

#[test]
fn walker_missing_dir_returns_empty() {
    let root = Utf8PathBuf::from("/nonexistent/path/that/does/not/exist");
    let entries = walk(&root, &empty_globset(), None, false).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn walker_finds_nested_files() {
    let dir = TempDir::new().unwrap();
    write_file(&dir, "a.flac", b"data");
    write_file(&dir, "artist/album/track.flac", b"data2");

    let entries = walk_dir(&dir);
    assert_eq!(entries.len(), 2);

    let paths: Vec<_> = entries.iter().map(|e| e.rel.as_str()).collect();
    assert!(paths.contains(&"a.flac"), "root file missing");
    assert!(paths.contains(&"artist/album/track.flac"), "nested file missing");
}

#[test]
fn walker_entries_are_sorted() {
    let dir = TempDir::new().unwrap();
    write_file(&dir, "z.flac", b"z");
    write_file(&dir, "a.flac", b"a");
    write_file(&dir, "m.flac", b"m");

    let entries = walk_dir(&dir);
    let paths: Vec<_> = entries.iter().map(|e| e.rel.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "walker must return sorted entries");
}

#[test]
fn walker_exclude_glob_skips_matching_files() {
    let dir = TempDir::new().unwrap();
    write_file(&dir, "music/track.flac", b"flac");
    write_file(&dir, "music/cover.jpg",  b"jpg");
    write_file(&dir, ".DS_Store",        b"junk");

    let exclude = {
        let mut b = GlobSetBuilder::new();
        b.add(globset::Glob::new("**/*.jpg").unwrap());
        b.add(globset::Glob::new(".DS_Store").unwrap());
        b.build().unwrap()
    };
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    let entries = walk(&root, &exclude, None, false).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].rel.as_str(), "music/track.flac");
}

#[test]
fn walker_include_glob_filters_to_matching_only() {
    let dir = TempDir::new().unwrap();
    write_file(&dir, "track.flac", b"flac");
    write_file(&dir, "track.mp3",  b"mp3");
    write_file(&dir, "cover.jpg",  b"jpg");

    let include = {
        let mut b = GlobSetBuilder::new();
        b.add(globset::Glob::new("**/*.flac").unwrap());
        b.build().unwrap()
    };
    let root = Utf8PathBuf::from_path_buf(dir.path().to_owned()).unwrap();
    let entries = walk(&root, &empty_globset(), Some(&include), false).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].rel.as_str(), "track.flac");
}

#[test]
fn walker_records_correct_file_size() {
    let dir = TempDir::new().unwrap();
    write_file(&dir, "track.flac", &[0u8; 12345]);

    let entries = walk_dir(&dir);
    assert_eq!(entries[0].size, 12345);
}

#[test]
fn walker_handles_unicode_filenames() {
    let dir = TempDir::new().unwrap();
    // Unicode characters including right-single-quotation-mark (U+2019)
    write_file(&dir, "Therion - A\u{2019}Arab Zaraq/01.flac", b"data");

    let entries = walk_dir(&dir);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].rel.as_str().contains('\u{2019}'));
}

// ── Compare + Plan tests (with real mtime from filesystem) ───────────────────

#[test]
fn diff_empty_src_empty_dst() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);
    assert!(plan.entries.is_empty());
}

#[test]
fn diff_all_new_when_dst_is_empty() {
    let src = TempDir::new().unwrap();
    write_file(&src, "01.flac", &[0u8; 1000]);
    write_file(&src, "02.flac", &[0u8; 2000]);

    let dst = TempDir::new().unwrap();

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::New), 2);
    assert_eq!(plan.count(EntryKind::Orphan), 0);
    assert_eq!(plan.transfer_bytes(), 3000);
}

#[test]
fn diff_all_orphan_when_src_is_empty() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(&dst, "old.flac", &[0u8; 500]);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Orphan), 1);
    assert_eq!(plan.count(EntryKind::New), 0);
}

#[test]
fn diff_same_file_is_same() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let data = [0u8; 500];
    write_file(&src, "track.flac", &data);
    write_file(&dst, "track.flac", &data);
    // Align mtimes so they're identical.
    set_mtime(&src, "track.flac", 1_700_000_000);
    set_mtime(&dst, "track.flac", 1_700_000_000);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Same), 1);
    assert_eq!(plan.count(EntryKind::Modified), 0);
}

#[test]
fn diff_size_mismatch_is_modified() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(&src, "track.flac", &[0u8; 800]);
    write_file(&dst, "track.flac", &[0u8; 400]);
    set_mtime(&src, "track.flac", 1_700_000_000);
    set_mtime(&dst, "track.flac", 1_700_000_000);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Modified), 1);
}

#[test]
fn diff_mtime_beyond_fat32_tolerance_is_modified() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let data = [0u8; 300];
    write_file(&src, "track.flac", &data);
    write_file(&dst, "track.flac", &data);
    // 10 seconds apart — well beyond the 2 s FAT32 tolerance.
    set_mtime(&src, "track.flac", 1_700_000_010);
    set_mtime(&dst, "track.flac", 1_700_000_000);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Modified), 1);
}

#[test]
fn diff_mtime_within_fat32_tolerance_is_same() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let data = [0u8; 300];
    write_file(&src, "track.flac", &data);
    write_file(&dst, "track.flac", &data);
    // 1 second apart — within the 2 s FAT32 window.
    set_mtime(&src, "track.flac", 1_700_000_001);
    set_mtime(&dst, "track.flac", 1_700_000_000);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Same), 1);
    assert_eq!(plan.count(EntryKind::Modified), 0);
}

#[test]
fn diff_mixed_scenario() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();

    // Same: identical content + mtime
    write_file(&src, "same.flac",     &[1u8; 100]);
    write_file(&dst, "same.flac",     &[1u8; 100]);
    set_mtime(&src, "same.flac", 1_700_000_000);
    set_mtime(&dst, "same.flac", 1_700_000_000);

    // Modified: same size, stale mtime on dst
    write_file(&src, "modified.flac", &[2u8; 200]);
    write_file(&dst, "modified.flac", &[2u8; 200]);
    set_mtime(&src, "modified.flac", 1_700_000_100);
    set_mtime(&dst, "modified.flac", 1_700_000_000);

    // New: only in source
    write_file(&src, "new.flac",      &[3u8; 300]);

    // Orphan: only in destination
    write_file(&dst, "orphan.flac",   &[4u8; 400]);

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::New),      1, "new");
    assert_eq!(plan.count(EntryKind::Modified), 1, "modified");
    assert_eq!(plan.count(EntryKind::Orphan),   1, "orphan");
    assert_eq!(plan.count(EntryKind::Same),     1, "same");
    assert_eq!(plan.transfer_bytes(), 200 + 300, "transfer bytes = modified + new");
}

#[test]
fn diff_checksum_detects_silent_corruption() {
    // Same size, same mtime, DIFFERENT content — size+mtime would say Same,
    // but checksum must detect Modified.
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    write_file(&src, "track.flac", b"original audio data");
    write_file(&dst, "track.flac", b"corrupted_audio!!!!"); // same length
    set_mtime(&src, "track.flac", 1_700_000_000);
    set_mtime(&dst, "track.flac", 1_700_000_000);

    let se = walk_dir_hashed(&src);
    let de = walk_dir_hashed(&dst);

    // Verify hashes were computed.
    assert!(se[0].hash.is_some(), "src hash should be populated");
    assert!(de[0].hash.is_some(), "dst hash should be populated");
    assert_ne!(se[0].hash, de[0].hash, "hashes should differ for different content");

    let plan = compare(&se, &de, Verify::Checksum);
    assert_eq!(plan.count(EntryKind::Modified), 1, "checksum must detect content mismatch");
    assert_eq!(plan.count(EntryKind::Same), 0);
}

#[test]
fn diff_checksum_same_content_is_same() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let data = b"identical audio data";
    write_file(&src, "track.flac", data);
    write_file(&dst, "track.flac", data);
    set_mtime(&src, "track.flac", 1_700_000_000);
    set_mtime(&dst, "track.flac", 1_700_000_100); // different mtime — SizeMtime would say Modified

    let se = walk_dir_hashed(&src);
    let de = walk_dir_hashed(&dst);
    let plan = compare(&se, &de, Verify::Checksum);

    // Checksum trusts the content over mtime.
    assert_eq!(plan.count(EntryKind::Same), 1, "identical content → Same even with mtime drift");
    assert_eq!(plan.count(EntryKind::Modified), 0);
}

#[test]
fn diff_idempotent_after_sync() {
    // Simulates: after a full sync, running diff again → everything Same.
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let files = [("a/b.flac", 100u64), ("c/d.flac", 200u64)];

    for (name, size) in &files {
        let data = vec![0u8; *size as usize];
        write_file(&src, name, &data);
        write_file(&dst, name, &data);
        set_mtime(&src, name, 1_700_000_000);
        set_mtime(&dst, name, 1_700_000_000);
    }

    let se = walk_dir(&src);
    let de = walk_dir(&dst);
    let plan = compare(&se, &de, Verify::SizeMtime);

    assert_eq!(plan.count(EntryKind::Same), files.len());
    assert_eq!(plan.transfer_bytes(), 0);
}
