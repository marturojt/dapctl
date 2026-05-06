# Changelog

All notable changes to this project will be documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/).

## [Unreleased]

---

## [0.4.0] — 2026-05-05

### Added
- **Synced lyrics** (`player::lyrics`): auto-detects `.lrc` alongside the
  audio file, parses multi-timestamp LRC format, and scrolls the active line
  to ~⅓ from the top of the pane. `i` key toggles between queue and lyrics
  pane; hints update when lyrics are present.
- **Play history + resume position** (`player::history`): append-only JSONL
  log in the platform data dir. On opening a track that appears in history,
  playback resumes from the last recorded position.
- **Sleep timer**: `t` key cycles off → 15 → 30 → 45 → 60 min. A countdown
  badge `⏾ M:SS` appears in the HiFi line. On expiry the engine pauses and
  emits a flash notification.
- **Equalizer animation** in Now Playing: animated sin-wave bars
  (▁▂▃▄▅▆▇█ cycling through 8 phases) while playing; collapses to a muted
  flat line when paused.
- **`dapctl audit <path>`** — offline read-only library audit (`audit::scanner`
  + `audit::report`):
  - Detects missing tags (artist/album/title/track#/year), absent cover art
    (embedded or `folder.jpg`), format mix within an album, and track-number
    gaps.
  - Severity levels: high/medium/low. Flags `--min-severity` and `--limit`.
  - Human table + `--json` structured output.
- **`dapctl cover fetch <path> [--online]`** — batch cover art downloader:
  - Without `--online`: prints policy message and exits (code 2). No network
    calls ever happen unless explicitly opted in.
  - Pipeline: MusicBrainz WS2 search → Cover Art Archive → iTunes Search API
    fallback.
  - 30-day disk cache at `$XDG_CACHE_HOME/dapctl/metadata/cover_cache.json`.
  - Rate limits: 1.1 s between MusicBrainz requests, 3.5 s between iTunes.
  - Saves `folder.jpg` in each album directory; non-JPEG images are converted
    via the `image` crate.
- **`docs/NETWORK.md`** — documents all network endpoints, user-agent string,
  rate limits, cache TTL, and opt-in policy.

### Changed
- **Library normalization**: artist and album grouping keys are now
  case-insensitive and diacritic-insensitive (à/á/â/ä → a, ñ → n, í → i, …).
  "Kings Of Leon" / "kings of leon" and "Rosalía" / "Rosalia" now merge into
  one entry. Display name is the first value seen for each normalised key.
- **TUI UX improvements**:
  - Diff view: filter tabs rendered as a visual tab row with per-tab counts
    and active-tab highlight; replaces the plain header line.
  - New Profile wizard: dot step indicator `● ● ○ ○ ○` at the top of each
    step shows progress through the 5-step flow.
  - Profiles view: sync mode badge is coloured (mirror → warn amber, additive
    → muted green); last-sync indicator `✓ Xh ago` shown when available.
  - Player: repeat/shuffle state displayed inline in the HiFi metadata line
    (`↺` all-repeat · `↺1` one-repeat · `⇄` shuffle). Focused pane title
    shown in bold with `▶` prefix.
  - Queue pane title shows current position `(X/N)`.

### Dependencies added
- `reqwest 0.12` (blocking, native-tls) — cover fetch HTTP client.
- `image 0.25` (jpeg + png features) — JPEG conversion for downloaded covers.

---

## [0.3.0] — 2026-05-01

### Added
- Home landing screen (`tui::views::home`): figlet ASCII art banner,
  navigable menu (sync & profiles · player · log), connected DAPs status.
  Default entry point on `dapctl` launch; `q` from sub-views returns here.
- TUI audio player — open with `m` from the profiles screen:
  - Three-pane layout: library browser · now playing + queue · key hints.
  - **SQLite-backed library scanner** (`player::scanner`): tag-based grouping
    (`album_artist` → `artist` → path fallback). Cache in platform data dir
    keyed by mtime_ns + size; only re-reads changed files on subsequent opens.
  - Artist → album → track hierarchy with expand/collapse (`Enter`).
    `Tab` switches pane focus between library and queue.
  - `/` incremental search across artist · album · title (real-time filter).
  - **Gapless playback** via `TrackDoneNotifier<S>`: next decoder eagerly
    appended to the rodio Sink before the current source is exhausted;
    `Arc<AtomicBool>` fires on source end to drive a seamless queue advance.
  - HiFi metadata display in Now Playing: sample rate · bit depth · bitrate
    · channels populated by `lofty` on first play.
  - Volume (`+`/`-`), seek (`←`/`→` ±30 s), next/prev (`n`/`p`),
    shuffle (`s`), repeat cycle (`r`).
  - Source toggle (`L`/`D`): play from source library or mounted DAP
    destination. Paths resolved from the active sync profile.
  - Preview from diff view: `space` on any entry enqueues and opens the
    player without starting playback automatically.
  - DSD (DSF/DFF) via ffmpeg pipe → PCM f32le 176.4 kHz (requires ffmpeg
    in PATH; clear error message and auto-advance when absent).
- `PlayerCommand::LoadQueue` — populates queue without auto-starting
  playback (complements existing `PlayQueue`).
- `Queue::peek_next()` — returns the track `advance()` would play next
  without mutating state; used by gapless preload.

### Changed
- Default TUI entry point changed from `profiles` view to `home` view.
  `q` from profiles returns to home; `q` from home exits.

---

## [0.2.0] — 2026-04-28

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

[0.3.0]: https://github.com/marturojt/dapctl/releases/tag/v0.3.0
[0.2.0]: https://github.com/marturojt/dapctl/releases/tag/v0.2.0
[0.1.0]: https://github.com/marturojt/dapctl/releases/tag/v0.1.0
