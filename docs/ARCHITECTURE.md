# Architecture

## Goals

- **Correctness at the filesystem boundary**: a tool that mutates the
  user's microSD cannot silently corrupt it. Every mutation goes
  through `transfer::executor` and emits an event before and after.
- **CLI-first, TUI-equal**: the TUI is a front-end over the same core
  APIs the CLI calls. No logic lives only in the TUI.
- **Portability is a feature, not an afterthought**: platform-specific
  code is confined to `scan::removable` and the minimal set of
  filesystem primitives.

## Module layout

```
cli         → clap parser, subcommand dispatch, non-interactive glue
tui         → ratatui app, views, theme (reads from `core` only)
config      → sync profile TOML (user-authored)
dap         → DAP profile TOML (builtin + user overrides)
scan        → removable drive enumeration + DAP identification
diff        → walker, comparator, serialisable Plan
transfer    → executor (temp+rename), manifest (resume), verify
transcode   → ffmpeg detection, engine (spawns ffmpeg), blake3-keyed cache
export      → M3U playlist generation
logging     → tracing init with dual sinks (human + JSONL v1)
player      → rodio/symphonia engine, queue, decoder, library index,
              lyrics (.lrc), history/resume, SQLite tag cache
audit       → offline library health scanner + structured report
cover       → cover art fetch (MusicBrainz/CAA/iTunes) + tag embed (lofty)
ssh         → SSH source adapter (system `ssh` binary, zero new deps)
error       → typed thiserror enums: ConfigError, DapError, ScanError
```

### Dependency direction

```
cli, tui
   └──► config, dap, scan, diff, transfer, transcode, export,
        logging, player, audit, cover, ssh
                                 └──► (stdlib, serde, walkdir, blake3,
                                       lofty, rodio, symphonia, rusqlite,
                                       reqwest, image, ...)
```

`core` modules must not depend on `cli` or `tui`. Tests exercise each
`core` module directly without going through clap or crossterm.

## Data flow (happy path)

1. `cli::sync` parses `--profile`, loads the `SyncProfile`.
2. `dap::load` resolves the referenced `DapProfile`.
3. `scan` resolves `destination` (e.g. `auto:fiio-m21` → mount point).
4. `diff::walker` enumerates source and destination:
   - If `source` is an `ssh://` URI, delegates to `ssh::SshSession::walk()`
     instead of the local filesystem walker.
   - Applies glob exclude/include filters.
   - Applies tag filters (artist/genre/sample rate/bit depth via `lofty`)
     when any are configured. Unreadable files always pass.
   - Projects source extensions to target extensions for transcode rules
     (e.g. `song.dsf` → `song.flac`) before the destination diff.
   - Optionally computes a per-file blake3 hash when `verify = "checksum"`.
   - Emits `PathWarning` for filenames or paths that exceed the DAP
     firmware's declared limits (from the active DAP profile).
5. `diff::compare` merge-joins the sorted entry lists, classifying each
   file as New / Modified / Orphan / Same. Transcoded pairs use mtime-only
   staleness (size/checksum comparison across different formats is
   meaningless). The configured `Verify` policy applies to direct copies.
6. `cli` / `tui` presents the plan; destructive ops require confirmation
   unless `--yes` was passed.
7. `transfer::executor` executes the plan, updating the manifest and
   emitting events. Three copy paths:
   - **SSH source**: `ssh::SshSession::download()` streams `cat` stdout
     to a temp file, then renames into place.
   - **Transcode**: checks `transcode::Cache`; on miss runs ffmpeg and
     stores the result; then renames into place.
   - **Local copy**: direct `fs::copy` with temp+rename.
   Post-copy verification (size+mtime or checksum) runs for local copies.
8. `logging` writes events to human stream and JSONL stream.

## Key invariants

- Source files are never modified. Ever.
- A destructive step (overwrite, delete) is preceded by a manifest
  entry transition to `InProgress`. A crash leaves either `InProgress`
  (will be re-tried) or `Done`, never inconsistent.
- Unknown fields in a DAP profile TOML are a hard error
  (`deny_unknown_fields`). Silent acceptance of unknown quirks is how
  library corruption ships.
- Every subcommand has a machine-readable output mode (`--json`) so
  automation does not have to parse human prose.
- Exit codes: 0 = success, 1 = generic error, 2 = config/DAP error
  (`ConfigError` / `DapError`), 3 = scan/environment error (`ScanError`).

## Resume protocol (manifest)

Per-run file at `$XDG_STATE_HOME/dapctl/runs/<ulid>.jsonl`.

```
{"path":"Tool/Lateralus/01.flac","size_bytes":86312448,"state":"pending"}
{"path":"Tool/Lateralus/01.flac","size_bytes":86312448,"state":"in_progress"}
{"path":"Tool/Lateralus/01.flac","size_bytes":86312448,"state":"done"}
```

On resume, `dapctl` reads the latest line for each path and re-queues
anything not in `Done`. For `Done` entries, it re-verifies with the
profile's configured `verify` policy before skipping.

## JSONL log schema v1

Every event includes `schema_version`, `ts` (RFC3339), `level`,
`run_id` (ulid), and `event` (enum). Optional fields: `path`, `bytes`,
`err`. The first event of a run has `event="start"` and carries the
resolved profile digest, so replaying events is reproducible.

## SSH source

`source = "ssh://[user@]host[:port]/absolute/path"` in sync profiles.

- Authentication: fully delegated to the system `ssh` binary —
  `~/.ssh/config`, identity files, `ssh-agent`, and `known_hosts`
  all work with no extra configuration. `BatchMode=yes` prevents
  interactive password prompts inside the TUI.
- Remote requirements: GNU `find` with `-printf` (standard on Linux;
  on macOS `brew install findutils`); `cat` (universal).
- `SshSession::connect()` verifies reachability by running `ssh ... true`.
- `SshSession::walk()` runs `find <root> -type f -printf '%P\t%s\t%T@\n'`
  and parses tab-separated output. Glob filtering is applied client-side.
- `SshSession::download()` streams `cat <remote_path>` stdout directly
  to a local temp file in 256 KB chunks, calling `on_progress` per chunk.

## Player architecture

The TUI player (`tui::views::player`) is a three-pane layout backed by:

- `player::engine` — `rodio::Sink` + mpsc `PlayerCommand`/`PlayerEvent`
  channel. Runs in a dedicated thread. Gapless playback via eager
  preload of the next decoder into the sink.
- `player::decoder` — routes to symphonia (FLAC/MP3/ALAC/AAC/OGG/WAV)
  or an ffmpeg pipe (DSD) depending on file format.
- `player::queue` — playlist, shuffle, repeat (Off/All/One).
- `player::library` — SQLite-backed tag scanner (`player::scanner`)
  with mtime+size invalidation; in-memory `LibraryIndex` grouped by
  album_artist → album, normalised (case + diacritics).
- `player::history` — append-only JSONL in the platform data dir;
  resume position persisted on track change.
- `player::lyrics` — parses `.lrc` alongside audio, exposes
  `current_idx(position_ms)` for TUI auto-scroll.
