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

## Milestone 1 — v0.1 MVP  ·  all 11 requirements  ·  *done (released 2026-04-27)*

Released as `v0.1.0`. Validated: 2,108 FLAC · 75 GB · HiBy R4 microSD.

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
- [x] `Verify::Checksum` with streaming `blake3` (v0.2 — landed on main).
- [ ] Filesystem-aware path checks: warn on names exceeding DAP limits.

### Transfer  (req 5, 7, 9)

- [x] `additive` mode: copy new + modified, never delete.
- [x] `mirror` mode: dry-run mandatory in absence of `--yes`.
- [x] Executor with temp + fsync + rename; FAT32 caveat documented.
- [x] Manifest JSONL per run at `%APPDATA%/dapctl/runs/<ulid>.jsonl`.
- [x] `indicatif` MultiProgress bars (overall + per-file, speed, ETA).
- [x] `dapctl sync <profile>` CLI with `--yes` / `--dry-run`, result summary.
- [x] Source mtime preserved on destination after rename (re-run idempotency).
- [x] `repair_dest_mtimes` pre-flight: fixes existing destinations in seconds.
- [x] Validated: 2,108 FLAC · 75 GB · HiBy R4 microSD · mirror + additive.
- [ ] `selective` mode: read `[selective]` from sync profile TOML,
      TUI writes back via `toml_edit` preserving comments.
- [ ] Manifest resume: on re-run, skip `Done` entries (currently re-diffs
      cleanly via temp file exclusion — full resume is a v0.2 refinement).

### TUI  (req 4, 5)

- [x] Event loop + `crossterm` input handling.
- [x] View: `profiles` — list profiles + connected DAPs, j/k navigation.
- [x] View: `diff` — summary table, filterable entry list (tab cycles
      All/New/Modified/Orphan/Same), j/k scroll, `y` to confirm sync,
      two-press confirmation for mirror mode with orphan deletions.
- [x] View: `progress` — overall gauge, per-file gauge with filename,
      speed / ETA / counters, auto-scrolling recent-events tail,
      completion banner. Sync runs in background thread via `mpsc`.
- [x] View: `new_profile` wizard — 4-step guided creation (name → source →
      destination with file browser + drive enumeration → mode). Duplicate
      name detection at step 1. `c` clones a profile pre-filled with
      `<name>-copy`. Writes `.toml` via `toml_edit`.
- [x] View: `log` — scrollable JSONL run viewer. j/k / g/G / r reload.
      Accessible via `l` from profiles or after sync completes.
- [ ] Theme plumbing: respect `NO_COLOR`, load palette from config.

### Tests

- [x] 10 unit tests for `diff::compare` (merge-join, FAT32 tolerance, Verify variants).
- [x] 19 integration tests for the diff pipeline (walker + compare + plan).
- [x] 4 integration tests for checksum verification (silent corruption detection).
- [x] 4 integration tests for tag filter graceful degradation.
- [ ] Manifest resume integration test (kill + restart).
- [ ] Snapshot tests (`insta`) for TUI views.
- [x] CI matrix: Linux x86_64/ARM64, macOS, Windows.

### Release engineering

- [x] `release.yml`: Linux musl x86_64+ARM64 via `cargo-zigbuild`,
      macOS universal (lipo), Windows MSVC. Draft release + SHA256SUMS.
- [x] Homebrew tap: `github.com/marturojt/homebrew-tap` — `brew tap marturojt/tap && brew install dapctl`.
- [ ] Scoop bucket skeleton.
- [ ] AUR `PKGBUILD` (git + bin variants).

---

## Milestone 2 — v0.2 Transcoding & metadata  ·  *done (released 2026-04-28)*

- [x] `Verify::Checksum` — streaming blake3 in both diff (per-walk hash) and
      transfer (post-copy verify). Silent corruption detected even when size
      and mtime match. Falls back to mtime when hashes not computed.
- [x] Tag filters (`lofty`) — `include_artists`, `exclude_artists`,
      `include_genres`, `exclude_genres`, `min_sample_rate_hz`,
      `max_sample_rate_hz`, `min_bit_depth` in sync profile `[filters]`.
      Unreadable files always pass (graceful degradation).
- [x] ffmpeg detection + capability probe (`transcode::ffmpeg::detect()`).
- [x] Transcode cache under `$XDG_CACHE_HOME/dapctl/transcode/`
      (256-shard blake3-keyed layout; `Cache::get` / `Cache::store`).
- [x] Rule language: `from`/`to`/`params` in sync profile `[transcode]`.
      Extension projection in walker (src ext → dst ext before diff);
      mtime-only staleness check across formats in compare.
      Executor uses cache on hit, runs ffmpeg on miss, stores result.
- [x] `dapctl export m3u <profile> [--output PATH]` — walks source with
      same filters as sync, prefixes paths with `dap.layout.music_root`.

## Milestone 3 — v0.3 TUI player + audit + cover fetch

**Philosophical scope expansion** — approved 2026-04-24. See plan §12
for detailed architecture, crate choices, and sub-milestones.

### 12-a · Player core  (est. 4–6 weeks)

- [ ] `player::engine` — rodio::Sink management, mpsc channels for
      `PlayerCommand` / `PlayerEvent`. Position via `Sink::get_pos()`.
- [ ] `player::decoder` — symphonia backend via rodio feature flags;
      handles FLAC/MP3/AAC/OGG/WAV/ALAC natively, zero ffmpeg dependency.
- [ ] `player::queue` — playlist, queue, shuffle/repeat.
- [ ] `tui::views::player` — 5th Ratatui view: Now Playing + barra de
      progreso + cola. Toggle `L`/`D` alterna source library / destino.
- [ ] Add `rodio` (symphonia-all feature) to Cargo.toml.

### 12-b · Player DSD + diff integration  (est. 2 weeks)

- [ ] DSD (DSF/DFF) via ffmpeg pipe → PCM 24/176.4 → rodio. Único caso
      que requiere ffmpeg. ⚠ icon + mensaje claro cuando no está en PATH.
- [ ] `space` keybind en diff view → push a cola del player → abre vista
      player. Flujo "escucha antes de sincronizar".
- [ ] Hi-res passthrough best-effort; WASAPI exclusive documentado como v1.0.

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
- [ ] Add `reqwest` (blocking) and `image` to Cargo.toml.
- [x] `lofty` already in Cargo.toml since v0.2.

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
