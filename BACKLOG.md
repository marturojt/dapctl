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
- [ ] Error taxonomy (`thiserror`) with user-facing messages vs internal. (v1.0)

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
- [x] Linux: `lsblk -o LABEL,MOUNTPOINT -P -n` for volume label detection.
- [x] macOS: `diskutil info <mount>` for volume label detection.

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
- [x] Filesystem-aware path checks: warn on names exceeding DAP limits.
      `PathWarning` / `check_path_limits` in diff; shown in CLI + TUI.

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
- [x] `selective` mode: read `[selective]` from sync profile TOML,
      TUI writes back via `toml_edit` preserving comments.
      `x` key toggles album-level selection (◆/◇). First open defaults
      to all albums selected. Wizard offers selective as 3rd mode option.
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
- [x] Theme: `NO_COLOR` environment variable respected (https://no-color.org);
      `Theme::new()` collapses to terminal defaults when set.

### Tests

- [x] 10 unit tests for `diff::compare` (merge-join, FAT32 tolerance, Verify variants).
- [x] 19 integration tests for the diff pipeline (walker + compare + plan).
- [x] 4 integration tests for checksum verification (silent corruption detection).
- [x] 4 integration tests for tag filter graceful degradation.
- [ ] Manifest resume integration test (kill + restart).
- [x] Snapshot tests (`insta`) for plan serialisation, path-limit logic,
      and DAP catalogue. `tests/snapshots.rs` (10 tests).
- [x] CI matrix: Linux x86_64/ARM64, macOS, Windows.

### Release engineering

- [x] `release.yml`: Linux glibc x86_64+ARM64 (native runners),
      macOS universal (lipo x86_64+aarch64), Windows MSVC. Draft release + SHA256SUMS.
- [x] Homebrew tap: `github.com/marturojt/homebrew-tap` — `brew tap marturojt/tap && brew install dapctl`.
- [x] Scoop bucket skeleton — `packaging/scoop/dapctl.json` with autoupdate.
- [x] AUR `PKGBUILD` (bin variant) — `packaging/aur/PKGBUILD`.

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

## Milestone 3 — v0.3 TUI player  ·  *done (released 2026-05-01)*

**Player core, gapless, library browser, HiFi display, home screen.**
See CHANGELOG [0.3.0] for the full list.

### Player  ·  *done*

- [x] `player::engine` — rodio::Sink + mpsc `PlayerCommand`/`PlayerEvent`.
      Gapless via `TrackDoneNotifier<S>` (AtomicBool per source, eager preload).
- [x] `player::decoder` — symphonia via rodio for PCM; ffmpeg pipe for DSD.
- [x] `player::queue` — playlist, shuffle, repeat (Off/All/One),
      `peek_next()` for gapless lookahead.
- [x] `player::scanner` — SQLite-backed tag scanner with mtime_ns+size
      invalidation; rayon-parallel `with_tags()`; platform data dir cache.
- [x] `player::library` — tag-grouped index (album_artist → artist → path
      fallback); `LibraryIndex` with flat filtered view for search.
- [x] `tui::views::player` — three-pane layout (library · now playing+queue ·
      hints). HiFi display (sample rate · bit depth · bitrate · channels).
      `/` search, `Tab` focus, `L`/`D` source toggle, volume, seek.
- [x] `tui::views::home` — landing screen, ASCII art banner, navigable menu.
- [x] `PlayerCommand::LoadQueue` — populate queue without auto-play.
- [x] DSD via ffmpeg pipe + diff view preview (`space` to enqueue).
- [x] README, CHANGELOG, BACKLOG, website updated to v0.3.0.
- [x] `new_profile` wizard expanded to 5 steps (name → source → destination
      with file browser → mode → summary).

---

## Milestone 4 — v0.4 Player Tier 2 + Audit + Cover fetch  ·  *done (released 2026-05-05)*

### Player Tier 2

- [x] Play history + resume position — append-only JSONL in data dir.
      `player::history`, `finish_current()` in engine, resume seek on open.
- [x] Sleep timer — `Instant` deadline in engine loop. `t` key cycles
      off/15/30/45/60 min; `SleepTimerFired` pauses and flashes.
- [x] Equalizer animation in now-playing — sin-wave bars (▁▂▃▄▅▆▇█),
      collapses to muted flat when paused. Replaced album art approach.
      Note: album art via `ratatui-image` deferred — version incompatible
      with ratatui 0.29; revisit when upgrading ratatui.
- [x] Synced lyrics — parse `.lrc` alongside audio, scroll by timestamp.
      `player::lyrics` (`from_lrc`, `current_idx`), auto-scroll ⅓ from top,
      `i` key toggles queue/lyrics pane, hints update when lyrics present.
- [x] Library normalisation — `normalize_key()` in `player::library`:
      case-insensitive + diacritic-insensitive grouping (à/á/â→a, ñ→n, í→i…).
      Display name = first value seen; BTreeMap key = normalised form.
- [x] TUI UX improvements — diff filter tab row with per-tab counts;
      new_profile 5-dot step indicator; profiles mode badge coloured
      (mirror=warn, additive=muted); last-sync `✓ Xh ago` indicator;
      player HiFi line shows repeat (`↺`/`↺1`) and shuffle (`⇄`) state;
      focused pane title bold + `▶` prefix; queue shows `(X/N)` position.
- [x] README, CHANGELOG, BACKLOG, website updated to v0.4.0.

### Audit  ·  *done*

- [x] `audit::scanner` — walk library with `lofty`, group by album folder.
- [x] Detect: missing tags (artist/album/title/track#/year), no cover
      (embedded or folder.jpg), format mix, track number gaps.
- [x] `audit::report` — serialisable report struct.
- [x] `dapctl audit <path>` — human table + `--json`. Read-only, offline.
      Severity levels: high/med/low. `--min-severity` and `--limit` flags.

### Cover fetch

- [x] `cover::musicbrainz` — (artist, album) → MBID → Cover Art Archive.
      Rate: 1 req/s. Cache TTL 30 days in `$XDG_CACHE_HOME/dapctl/metadata/`.
- [x] `cover::itunes` — iTunes Search API fallback. Rate: 20 req/min.
- [x] Download to `<album>/folder.jpg`. JPEG conversion via `image` crate.
- [x] `dapctl cover fetch <path> [--online]` — offline by default, exits
      with policy message and code 2 when `--online` not passed.
- [x] `docs/NETWORK.md` — policy, user-agent, rate limits, opt-in.
- [x] Add `reqwest` (blocking, native-tls) and `image` to Cargo.toml.

---

## Milestone 5 — v1.0 Community profiles & SSH

- [ ] SSH source via `russh`.
- [x] At least 6 DAP profiles with fixtures in CI.
      7 builtins: fiio-m21, fiio-m11, ak-sr35, hiby-r6, shanling-m3ultra,
      ibasso-dx320, generic. CI validated via `all_builtin_profiles_parse`.
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
