# Changelog

All notable changes to this project will be documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/marturojt/dapctl/compare/HEAD...HEAD
