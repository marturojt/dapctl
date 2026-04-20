# BACKLOG

Planning artefact. Authoritative roadmap lives here until GitHub Issues
takes over. Tasks are grouped by milestone and ordered roughly by
dependency. Every v0.1 task maps to a functional requirement in the
approved plan (see `~/.claude/plans/proyecto-dapctl-typed-sunbeam.md`).

Legend: `[ ]` todo · `[~]` in progress · `[x]` done · `(req N)` maps to
requirement N of the MVP.

---

## Milestone 0 — Scaffolding  ·  *current*

- [x] Cargo manifest with dependency inventory
- [x] Module skeleton under `src/` compiling against stub implementations
- [x] Builtin DAP profile: `fiio-m21`, `generic`
- [x] Example sync profile
- [x] GPLv3 license file
- [x] README, CHANGELOG, BACKLOG, architecture doc, DAP profile spec
- [x] CI workflow (fmt · clippy · test · audit)
- [x] Verify `cargo check` clean on maintainer machine (rustc 1.95.0, 2026-04-19)
- [ ] Confirm `dapctl` namespace — dapctl.com acquired; pending: crates.io, AUR

---

## Milestone 1 — v0.1 MVP  ·  all 11 requirements

Target: first end-to-end sync to the author's FiiO M21 works, reliably,
on Linux + macOS + Windows.

### Foundations

- [x] `logging::init`: dual sink (human + JSONL), `run_id` propagation,
      schema v1 frozen. (req 8)
- [ ] `cli`: flesh out `--yes`, `--dry-run`, exit code convention. (req 10)
- [ ] Error taxonomy (`thiserror`) with user-facing messages vs internal.

### Config & DAP catalogue  (req 1, 3, 6)

- [x] `config::load(path)` with schema validation, helpful error spans.
- [x] `dap::load(id)` — builtin first, then XDG override; `deny_unknown_fields`.
- [x] Merge exclusions: `ResolvedProfile` + `build_exclude_set` / `build_include_set`.
- [x] `dapctl profile list` — DAP profiles + sync profiles.
- [x] `dapctl profile show <id>` — DAP profile details.
- [x] `dapctl profile check <path>` — validate sync profile + resolve DAP.

### Scan  (req 2)

- [ ] Linux: `/proc/mounts` + `lsblk --json` for label + fs + sizes.
- [ ] macOS: `diskutil info -plist` (stopgap), migrate to IOKit.
- [ ] Windows: `GetLogicalDrives` + `GetVolumeInformationW`.
- [ ] `scan::heuristic::identify` covering FiiO M21 ground truth.
- [ ] `dapctl scan --json` output.

### Diff  (req 4, 6)

- [ ] Parallel walk with `walkdir` + `rayon`, applying globset filters.
- [ ] `Verify::SizeMtime` comparator.
- [ ] `Verify::Checksum` with `blake3`, cached per mtime/size tuple.
- [ ] `Plan` serialisable to JSON (for TUI handoff and tests).
- [ ] Filesystem-aware path checks: warn on names exceeding DAP limits.

### Transfer  (req 5, 7, 9)

- [ ] `additive` mode: copy new + modified, never delete.
- [ ] `mirror` mode: dry-run mandatory in absence of `--yes`.
- [ ] `selective` mode: read `[selective]` from sync profile TOML,
      TUI writes back via `toml_edit` preserving comments.
- [ ] Executor with temp + fsync + rename; FAT32 caveat documented.
- [ ] Manifest JSONL per run; resume re-queues non-`Done` entries.
- [ ] `indicatif` progress bars for non-TUI runs.

### TUI  (req 4, 5)

- [ ] Event loop + `crossterm` input handling.
- [ ] View: `profiles` — list profiles + connected DAPs.
- [ ] View: `diff` — summary, filterable entry list, selective marking.
- [ ] View: `progress` — total + current + event tail.
- [ ] View: `log` — scroll/filter.
- [ ] Theme plumbing: respect `NO_COLOR`, load palette from config.

### Tests

- [ ] Diff unit tests with fixture libraries under `tests/fixtures/`.
- [ ] Manifest resume integration test (kill + restart).
- [ ] Snapshot tests (`insta`) for TUI views.
- [ ] CI matrix: Linux x86_64/ARM64, macOS, Windows.

### Release engineering

- [ ] `release.yml`: cross-compile via `cargo-zigbuild`, attach binaries.
- [ ] Homebrew tap skeleton.
- [ ] Scoop bucket skeleton.
- [ ] AUR `PKGBUILD` (git + bin variants).

---

## Milestone 2 — v0.2 Transcoding & metadata

- [ ] ffmpeg detection + capability probe.
- [ ] Transcode cache under `$XDG_CACHE_HOME/dapctl/transcode/`.
- [ ] Rule language: `from`/`to`/`params` in sync profile `[transcode]`.
- [ ] M3U export with path rewrites for the DAP.
- [ ] Tag reading (`lofty`) for filters: artist / genre / bitdepth / SR.
- [ ] Post-copy checksum verification toggle.

## Milestone 3 — v1.0 Community profiles & SSH

- [ ] SSH source via `russh`.
- [ ] At least 6 DAP profiles with fixtures in CI.
- [ ] AcoustID duplicate detection (optional, `chromaprint`).
- [ ] Beets query integration as filter source.
- [ ] Official distribution: Homebrew core, Scoop, AUR, GH Releases.

---

## Icebox / not in scope

- Bidirectional sync (Syncthing territory).
- GUI of any kind.
- Cloud music service integration.
- Library tagging / organisation (Picard, beets).
- Non-audio file types.
