# BACKLOG

Planning artefact. Authoritative roadmap lives here until GitHub Issues
takes over. Tasks are grouped by milestone and ordered roughly by
dependency. Every v0.1 task maps to a functional requirement in the
approved plan (see `~/.claude/plans/proyecto-dapctl-typed-sunbeam.md`).

Legend: `[ ]` todo · `[~]` in progress · `[x]` done · `(req N)` maps to
requirement N of the MVP.

---

## Milestone 0 — Scaffolding  ·  *done*

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

## Milestone 1 — v0.1 MVP  ·  all 11 requirements  ·  *in progress*

Target: first end-to-end sync to the author's FiiO M21 works, reliably,
on Linux + macOS + Windows.

### Foundations

- [x] `logging::init`: dual sink (human + JSONL), `run_id` propagation,
      schema v1 frozen. (req 8)
- [x] `cli`: `--yes`, `--dry-run`, exit code convention. (req 10)
- [ ] Error taxonomy (`thiserror`) with user-facing messages vs internal.

### Config & DAP catalogue  (req 1, 3, 6)

- [x] `config::load(path)` with schema validation, helpful error spans.
- [x] `dap::load(id)` — builtin first, then XDG override; `deny_unknown_fields`.
- [x] Merge exclusions: `ResolvedProfile` + `build_exclude_set` / `build_include_set`.
- [x] `dapctl profile list` — DAP profiles + sync profiles.
- [x] `dapctl profile show <id>` — DAP profile details.
- [x] `dapctl profile check <path>` — validate sync profile + resolve DAP.

### Scan  (req 2)

- [x] `scan::removable::enumerate()` via `sysinfo` (cross-platform).
- [x] `scan::heuristic::identify()`: exact label, partial label, marker
      files (`.database_uuid`, `HiByMusic/`), exFAT/FAT32 fallback.
- [x] `scan::resolve_destination()` for `auto:<dap-id>` in sync profiles.
- [x] `dapctl scan` human table + `--json` output + MTP guidance message.
- [x] Windows: `GetVolumeInformationW` for correct volume label.
- [x] Heuristic covers FiiO M11 family, Shanling, iBasso, Cayin, HiBy.
- [ ] Linux: enhance with `lsblk --json` for more reliable label detection.
- [ ] macOS: `diskutil info -plist` for stricter removable detection.

**Note:** MTP connections are not supported by design. The primary
workflow is microSD extraction + card reader. See README for rationale.

### Diff  (req 4, 6)

- [x] `diff::walker::walk()` with globset exclude+include filtering,
      `/`-normalized paths for cross-platform glob matching.
- [x] `diff::compare::compare()` merge-join, Verify::SizeMtime with
      2 s FAT32 mtime tolerance.
- [x] `Plan` serialisable to JSON; `transfer_bytes()`, `eta_secs()`.
- [x] `diff::diff()` high-level entry point used by CLI and TUI.
- [x] `dapctl diff <profile>` human summary + entry list + `--json`.
- [ ] `Verify::Checksum` with `blake3` (v0.2).
- [ ] Filesystem-aware path checks: warn on names exceeding DAP limits.

### Transfer  (req 5, 7, 9)

- [x] `additive` mode: copy new + modified, never delete.
- [x] `mirror` mode: dry-run mandatory in absence of `--yes`.
- [x] Executor with temp + fsync + rename; FAT32 caveat documented.
- [x] Manifest JSONL per run at `%APPDATA%/dapctl/runs/<ulid>.jsonl`.
- [x] `indicatif` MultiProgress bars (overall + per-file, speed, ETA).
- [x] `dapctl sync <profile>` CLI with `--yes` / `--dry-run`, result summary.
- [ ] `selective` mode: read `[selective]` from sync profile TOML,
      TUI writes back via `toml_edit` preserving comments.
- [ ] Manifest resume: on re-run, skip `Done` entries (currently re-diffs
      cleanly via temp file exclusion — full resume is a v0.2 refinement).

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

## Milestone 3 — v0.3 TUI player + audit + cover fetch

**Philosophical scope expansion** — approved 2026-04-24. See plan §12
for detailed architecture, crate choices, and sub-milestones.

### 12-a · Player core  (est. 4–6 weeks)

- [ ] `player::engine` — rodio::Sink management, mpsc channels for
      `PlayerCommand` / `PlayerEvent`.
- [ ] `player::decoder` — symphonia for PCM (FLAC/MP3/ALAC/AAC/OGG/WAV),
      ffmpeg pipe router for DSD (same detection as v0.2 transcoding).
- [ ] `player::queue` — playlist, queue, shuffle/repeat.
- [ ] `tui::views::player` — 5th Ratatui view: Now Playing + Queue list
      + progress bar. Toggle `L`/`D` switches between source library
      and mounted destination.
- [ ] Add `rodio` + `symphonia` to Cargo.toml.

### 12-b · Player DSD + diff integration  (est. 2 weeks)

- [ ] DSD via ffmpeg pipe → PCM 24/176.4 → rodio. ⚠ icon when ffmpeg
      missing.
- [ ] `space` keybind in diff view → push to player queue → open player
      view. Pre-sync audio verification flow.
- [ ] Hi-res passthrough best-effort; document WASAPI exclusive as v1.0.

### 12-c · Audit  (est. 2 weeks)

- [ ] `audit::scanner` — walk library with `lofty`, group by album folder.
- [ ] Detect: missing tags (artist/album/title/track#/year), no cover
      (embedded or folder.jpg), format mix, track number gaps.
- [ ] `audit::report` — serialisable report struct.
- [ ] `dapctl audit <path>` — human table + `--json`. Read-only, offline.

### 12-d · Cover fetch  (est. 3 weeks)

- [ ] `cover::musicbrainz` — search by (artist, album) → MBID →
      Cover Art Archive fetch. Rate: 1 req/s.
- [ ] `cover::itunes` — iTunes Search API fallback (no key required).
      Rate: 20 req/min.
- [ ] Metadata cache at `$XDG_CACHE_HOME/dapctl/metadata/`, TTL 30 days.
- [ ] Download to `<album>/folder.jpg`. Resize to ≥600×600 JPEG.
      No tag embedding in v0.3.
- [ ] `dapctl cover fetch <path> [--online]` — fails with clear message
      without `--online`.
- [ ] `docs/NETWORK.md` — policy, user-agent, rate limits, opt-in.
- [ ] README "What dapctl is not" section updated to reflect v0.3 scope.
- [ ] Add `lofty`, `reqwest` (blocking), `image` to Cargo.toml.

---

## Milestone 4 — v1.0 Community profiles & SSH

- [ ] SSH source via `russh`.
- [ ] At least 6 DAP profiles with fixtures in CI.
- [ ] AcoustID duplicate detection (optional, `chromaprint`).
- [ ] Beets query integration as filter source.
- [ ] Cover art embed in tags (lofty write, all formats, v1.0).
- [ ] Official distribution: Homebrew core, Scoop, AUR, GH Releases.

---

## Icebox / not in scope

- Bidirectional sync (Syncthing territory).
- GUI of any kind.
- Cloud music service integration.
- Full tag editor (that's Picard or beets — dapctl audits, not edits).
- Scrobbling / smart playlists / EQ (player is audit, not library mgr).
- Non-audio file types.
