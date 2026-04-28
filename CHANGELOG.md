# Changelog

All notable changes to this project will be documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/).

## [Unreleased] — v0.2 in progress

### Added
- `Verify::Checksum` mode: streaming blake3 hash (1 MiB buffer) computed
  during the diff walk. Silent corruption is detected even when file size
  and mtime match. Falls back to mtime comparison when hashes are not
  present (i.e. when `verify = "size_mtime"` is configured).
  `transfer::verify::hash_file()` is now public for reuse by the executor's
  post-copy verification pass.
- Tag-based filters in sync profile `[filters]` section, powered by `lofty`:
  - `include_artists` / `exclude_artists` (case-insensitive)
  - `include_genres` / `exclude_genres` (case-insensitive)
  - `min_sample_rate_hz` / `max_sample_rate_hz`
  - `min_bit_depth`
  All new fields are optional and default to "no filter". Files that lofty
  cannot parse (DSD formats, non-audio, corrupt) always pass — they are
  never silently dropped.
- Transcode pipeline (`[transcode]` section in sync profile):
  - `transcode::ffmpeg::detect()` — probes for `ffmpeg` in PATH at
    startup; logs version when found.
  - Rule language: `[[transcode.rules]]` entries with `from`, `to`, and
    optional `params` fields.
  - Extension projection in the diff walker: source files matching a rule's
    `from` extension are projected to `to` before the destination diff,
    so `song.dsf` compares against `song.flac` on the DAP.
  - Mtime-only staleness for transcoded pairs (size/checksum comparison
    across different formats is meaningless); source mtime is preserved on
    the output after transcode so re-runs are idempotent.
  - Transcode cache at `$XDG_CACHE_HOME/dapctl/transcode/` with a
    256-shard blake3-keyed layout (`blake3(source_content || params)`).
    Cache hit avoids re-running ffmpeg; failure to write cache is non-fatal.
  - `TranscodeOpts` wired into `executor::Options`; `cli::sync` builds it
    from `ProjectDirs` when `transcode.enabled = true` and rules are present.
- `dapctl export m3u <profile> [--output PATH]`: walks the source with the
  same filters as `dapctl sync`, prefixes each path with the DAP's
  `layout.music_root`, and emits a standard `#EXTM3U` playlist. Write to
  file or stdout.

### Tests
- 4 new unit tests for `Verify::Checksum` classify paths (same hash,
  different hash same size, no-hash mtime fallback).
- 3 new unit tests for transcode classify paths (dst newer → Same,
  src newer → Modified, new entry carries `transcode_from`).
- 2 new integration tests: `diff_checksum_detects_silent_corruption`
  (same size + mtime, different content → Modified) and
  `diff_checksum_same_content_is_same` (different mtime, same content →
  Same). Total: 39 tests.
- 4 new tag-filter integration tests covering inactive-by-default,
  graceful degradation on unreadable files, and sample-rate / artist
  filter activation.

---

## [0.1.0] — 2026-04-27

### Added
- Repository scaffolding: Cargo manifest, module layout (`cli`, `config`,
  `dap`, `scan`, `diff`, `transfer`, `logging`, `tui`), builtin DAP
  profiles for FiiO M21 and a generic fallback.
- GPLv3 license, README, architecture docs, backlog, CI workflow.
- `logging::init`: dual-sink tracing (human stderr/file + JSONL v1).
  Per-run `run_id` (ULID), schema v1 frozen, `finish` event on exit.
- `config::load` / `config::resolve`: sync profile TOML parsing with
  schema validation, glob validation, and `ResolvedProfile` (merged
  exclude/include globsets from DAP + sync profile).
- `dap::load`: builtin catalogue (`fiio-m21`, `generic`) with XDG
  user-override support. `dapctl profile list/show/check` commands.
- `scan::removable`: sysinfo-based removable drive enumeration.
  Windows: `GetVolumeInformationW` for correct volume labels.
- `scan::heuristic`: DAP identification by label, marker files
  (`.database_uuid`, `HiByMusic/`), and exFAT/FAT32 fallback.
  Covers FiiO M11 family, Shanling, iBasso, Cayin, HiBy, AK.
- `scan::resolve_destination`: resolves `auto:<dap-id>` to a mount path.
- `dapctl scan`: human table + `--json`. MTP guidance in empty state.
- `diff::walker`: recursive walk with globset filtering, `/`-normalised
  paths for cross-platform glob matching.
- `diff::compare`: O(n) merge-join with 2 s FAT32 mtime tolerance.
- `diff::Plan`: `count()`, `total_bytes()`, `transfer_bytes()`,
  `eta_secs()`. Fully serialisable.
- `dapctl diff <profile>`: summary table with ETA + first 40 entries.
  `--json` emits full Plan.
- `transfer::executor`: temp + fsync + rename copy pipeline; `indicatif`
  MultiProgress bars (overall + per-file, speed, ETA).
- `transfer::manifest`: append-only JSONL per run at
  `%APPDATA%/dapctl/runs/<ulid>.jsonl`.
- `transfer::verify`: size+mtime (2 s FAT32 tolerance) and blake3
  checksum modes.
- `dapctl sync <profile>`: additive and mirror modes, `--yes` /
  `--dry-run`, result summary, exit code 0/1.
- TUI diff view: summary table (counts + bytes + ETA per kind), entry
  list with `tab`-cycled filter (All / New / Modified / Orphan / Same),
  j/k scroll, color-coded entry icons.
- TUI sync from diff view: `y` confirms and launches sync; mirror mode
  with orphans requires a second `y` with a flash warning.
- `transfer::ProgressEvent` channel: executor optionally sends
  `FileStart / FileProgress / FileDone / FileFail / DeleteDone / Finish`
  to a `mpsc::Sender` so the TUI can consume them without indicatif
  terminal output.
- TUI progress view: overall `Gauge` (done/total bytes + %), per-file
  `Gauge` with filename in title, live speed / ETA / copied / deleted /
  failed stats, auto-scrolling recent-events tail (last 200), completion
  banner. Sync executes in a background thread; main thread drains the
  channel each frame.

### Fixed
- `transfer::executor`: preserve source mtime on destination after
  rename so `diff` correctly classifies files as `Same` on re-runs and
  the post-copy `size_mtime` verify passes. Without this, every re-run
  re-transferred the entire library.
- `cli::sync`: run a `repair_dest_mtimes` pass before `diff` to fix
  existing destinations that were populated before the mtime-preservation
  fix (metadata-only, takes seconds regardless of library size).

### Changed
- `release.yml`: cross-compile pipeline triggered on `v*` tags. Produces
  Linux musl static binaries (x86_64 + aarch64) via `cargo-zigbuild`,
  a macOS universal binary via `lipo`, and a Windows MSVC zip. All
  artifacts attached to a draft GitHub release with `SHA256SUMS.txt`.

### Added (TUI)
- TUI new-profile wizard: 4-step guided creation (name → source →
  destination → mode). File browser with drive enumeration on Windows,
  duplicate name detection at step 1. `c` clones a selected profile
  pre-filled with suggested `<name>-copy`.
- TUI log view: scrollable display of the most recent JSONL run.
  j/k scroll, `g`/`G` top/bottom, `r` reload. Accessible via `l` from
  profiles or from progress view after sync completes.
- Key-repeat fix: only `KeyEventKind::Press` events are processed;
  OS repeat events are discarded, eliminating runaway scrolling.

### Fixed
- Wizard TOML writer: escape backslashes in Windows paths
  (`D:\Music` → `D:\\Music`) — unescaped paths caused silent parse
  failure and profiles never appeared in the list.
- `transfer::executor::truncate_path`: slice by char index instead of
  byte offset — panicked on Unicode characters such as U+2019 (`'`)
  in album/folder names.
- Wizard `verify` field: was written as `"size+mtime"` but the schema
  expects `"size_mtime"` (snake_case) — profiles were silently dropped.

### Tests
- 10 unit tests for `diff::compare` (merge-join logic, FAT32 mtime
  tolerance, `Verify::None` behaviour).
- 17 integration tests for the diff pipeline (`walker` + `compare`):
  glob filtering, Unicode filenames, mtime alignment, idempotency.

### Validated
- End-to-end sync against HiBy R4 microSD (F:\\, exFAT, 116 GB):
  2,108 FLAC files, 75.1 GB transferred and verified.
- Mirror mode: orphan detection and deletion confirmed.
- Re-run idempotency: 2,108 unchanged, 0 copied, 0 failed.

[0.1.0]: https://github.com/marturojt/dapctl/releases/tag/v0.1.0
