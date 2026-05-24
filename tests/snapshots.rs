//! Snapshot tests (insta) for plan serialisation, path-limit checks,
//! and the DAP profile catalogue.

use camino::Utf8PathBuf;

use dapctl::diff::plan::{Entry, PathWarning, PathWarningKind};
use dapctl::diff::{check_path_limits, EntryKind, Plan};

// ── helpers ──────────────────────────────────────────────────────────────────

fn mk_entry(kind: EntryKind, path: &str, size_bytes: u64) -> Entry {
    Entry {
        kind,
        path: Utf8PathBuf::from(path),
        size_bytes,
        transcode_from: None,
    }
}

fn fat32_fs() -> dapctl::dap::Filesystem {
    dapctl::dap::Filesystem {
        preferred: "exFAT".into(),
        supported: vec!["FAT32".into(), "exFAT".into()],
        max_filename_bytes: 255,
        max_path_bytes: 4096,
        case_sensitive: false,
    }
}

/// Artificially tight limits to exercise both warning kinds in small tests.
fn tight_fs() -> dapctl::dap::Filesystem {
    dapctl::dap::Filesystem {
        preferred: "FAT32".into(),
        supported: vec!["FAT32".into()],
        max_filename_bytes: 64,
        max_path_bytes: 128,
        case_sensitive: false,
    }
}

// ── Plan JSON snapshot ────────────────────────────────────────────────────────

#[test]
fn plan_json_all_entry_kinds() {
    let plan = Plan {
        entries: vec![
            mk_entry(
                EntryKind::New,
                "Tool/Lateralus/01 - The Grudge.flac",
                90_508_288,
            ),
            mk_entry(
                EntryKind::Modified,
                "Radiohead/OK Computer/06 - Karma Police.flac",
                33_685_504,
            ),
            mk_entry(
                EntryKind::Orphan,
                "Nickelback/How You Remind Me.mp3",
                5_242_880,
            ),
            mk_entry(
                EntryKind::Same,
                "Portishead/Dummy/03 - Strangers.flac",
                52_428_800,
            ),
        ],
        warnings: vec![],
    };
    insta::assert_json_snapshot!(&plan);
}

#[test]
fn plan_json_with_warnings() {
    let long_name = "A".repeat(260);
    let path = format!("Artist/Album/{long_name}.flac");
    let plan = Plan {
        entries: vec![mk_entry(EntryKind::New, &path, 45_000_000)],
        warnings: vec![PathWarning {
            path: Utf8PathBuf::from(&path),
            kind: PathWarningKind::FilenameTooLong,
            length_bytes: 265, // 260 'A' + ".flac"
            limit_bytes: 255,
        }],
    };
    insta::assert_json_snapshot!(&plan);
}

// ── check_path_limits unit tests ─────────────────────────────────────────────

#[test]
fn no_warnings_for_normal_paths() {
    let entries = vec![
        mk_entry(EntryKind::New, "Artist/Album/01 - Track.flac", 1000),
        mk_entry(
            EntryKind::Modified,
            "Other Artist/Record/02 - Song.flac",
            2000,
        ),
    ];
    let warnings = check_path_limits(&entries, &fat32_fs(), "/Music");
    assert!(
        warnings.is_empty(),
        "expected no warnings, got {warnings:?}"
    );
}

#[test]
fn warns_on_filename_too_long() {
    let long = "A".repeat(260); // 260 bytes > 255 limit
    let entries = vec![mk_entry(
        EntryKind::New,
        &format!("Artist/Album/{long}.flac"),
        1000,
    )];
    let warnings = check_path_limits(&entries, &fat32_fs(), "/Music");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].kind, PathWarningKind::FilenameTooLong);
    assert!(warnings[0].length_bytes > 255);
    insta::assert_debug_snapshot!(warnings);
}

#[test]
fn warns_on_path_too_long() {
    // tight_fs: max_path_bytes=128, max_filename_bytes=64.
    // seg="d"*30 (30 bytes < 64): four segments + sep + "t.flac"
    // relative = 30+1+30+1+30+1+30+1+6 = 130 bytes
    // full = "/Music" (6) + "/" (1) + 130 = 137 > 128 ✓
    let seg = "d".repeat(30);
    let entries = vec![mk_entry(
        EntryKind::New,
        &format!("{seg}/{seg}/{seg}/{seg}/t.flac"),
        1000,
    )];
    let warnings = check_path_limits(&entries, &tight_fs(), "/Music");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].kind, PathWarningKind::PathTooLong);
    insta::assert_debug_snapshot!(warnings);
}

#[test]
fn only_one_warning_per_entry() {
    // Filename is too long — should produce exactly one warning, not two.
    let long = "B".repeat(70); // 70 bytes > 64 tight_fs limit
    let entries = vec![mk_entry(EntryKind::New, &format!("Dir/{long}.flac"), 1000)];
    let warnings = check_path_limits(&entries, &tight_fs(), "/Music");
    assert_eq!(warnings.len(), 1, "expected exactly one warning per entry");
    assert_eq!(warnings[0].kind, PathWarningKind::FilenameTooLong);
}

#[test]
fn skips_orphan_and_same_entries() {
    let long = "X".repeat(260);
    let entries = vec![
        mk_entry(EntryKind::Orphan, &format!("Dir/{long}.flac"), 1000),
        mk_entry(EntryKind::Same, &format!("Dir/{long}.flac"), 1000),
    ];
    let warnings = check_path_limits(&entries, &fat32_fs(), "/Music");
    assert!(
        warnings.is_empty(),
        "orphan/same entries must not generate warnings"
    );
}

// ── Builtin profile catalogue ─────────────────────────────────────────────────

#[test]
fn all_builtin_profiles_parse() {
    // Only check compiled-in profiles; user overrides in the config dir are
    // not under our control and may be malformed without that being a bug here.
    for (id, toml_str) in dapctl::dap::builtin::ALL {
        let result: Result<dapctl::dap::DapProfile, _> = toml::from_str(toml_str);
        assert!(
            result.is_ok(),
            "builtin profile {id:?} failed to parse: {:?}",
            result.err()
        );
    }
}

#[test]
fn builtin_profile_count_at_least_seven() {
    assert!(
        dapctl::dap::builtin::ALL.len() >= 7,
        "expected at least 7 builtin profiles, found {}",
        dapctl::dap::builtin::ALL.len()
    );
}

#[test]
fn builtin_profile_ids_snapshot() {
    let mut ids: Vec<&str> = dapctl::dap::builtin::ALL
        .iter()
        .map(|(id, _)| *id)
        .collect();
    ids.sort();
    insta::assert_debug_snapshot!(ids);
}
