# dapctl

> TUI/CLI sync tool for HiFi Digital Audio Players.
> `systemctl`-style subcommands. Rust + Ratatui. GPLv3.

`dapctl` is a terminal-first sync tool that knows about your DAP. It
compares your music library against the destination, shows a clear diff,
and copies only what's needed — respecting per-device quirks: filesystem
limits, supported codecs, cache folders to exclude.

## Status

**v1.0.0 released.**

v1.0 adds: SSH source (`ssh://[user@]host/path` — sync from a remote library with
zero extra dependencies), `dapctl cover embed` (write cover art into FLAC/MP3/M4A/OGG/Opus
tags from `folder.jpg`), `dapctl profile delete` (CLI + TUI `D` key with two-press
confirm), 5 new builtin DAP profiles (fiio-m11, ak-sr35, hiby-r6, shanling-m3ultra,
ibasso-dx320 — 7 total), path-limit warnings from DAP firmware spec, `NO_COLOR` support,
typed error taxonomy with exit codes 2/3, selective mode write-back, snapshot tests.

v0.4 added: synced lyrics (`.lrc` auto-scroll, `i` toggle), play
history + resume position, sleep timer, equalizer animation, library
normalisation (case + diacritics — "Rosalía" and "Rosalia" merge),
`dapctl audit` (offline library health: missing tags, absent covers,
format mix, track-number gaps), `dapctl cover fetch` (MusicBrainz →
Cover Art Archive → iTunes, opt-in `--online`, 30-day cache), TUI UX
improvements (diff tab row, wizard step dots, mode badges, last-sync
indicator).

v0.3 added: TUI audio player with SQLite-backed library browser
(artist → album → track, tag-grouped), gapless playback, HiFi metadata
display (sample rate · bit depth · bitrate · channels), `/` incremental
search, source toggle library ↔ DAP destination, home landing screen.

v0.2 added: blake3 checksum verification, tag-based filters (artist ·
genre · sample rate · bit depth), ffmpeg transcode pipeline with a
blake3-keyed cache, `dapctl export m3u`.

v0.1 was real-world validated: 2,108 FLAC · 75 GB · HiBy R4 microSD.

See [`BACKLOG.md`](BACKLOG.md) and [`CHANGELOG.md`](CHANGELOG.md).

## How it works

dapctl is designed around the **microSD card reader** workflow:

1. Extract the microSD from your DAP.
2. Insert it into a USB card reader — it mounts as a regular drive.
3. Run `dapctl scan` — dapctl identifies the device by its volume label
   and known firmware markers.
4. Run `dapctl diff <profile>` — see exactly what would change.
5. Run `dapctl sync <profile>` — copy only what's needed.

This is the workflow most audiophiles already use for large library
syncs. It is faster (~40–80 MB/s) and more reliable than any
USB-connected transfer mode.

> **MTP is not supported natively.** Android-based DAPs connected via
> USB appear as `This PC\Device Name\...` — not mounted drives.
> You can work around this by pre-mounting with `jmtpfs` (Linux/macOS)
> or `rclone` + WinFsp (Windows). See [`docs/MTP_WORKFLOW.md`](docs/MTP_WORKFLOW.md).
> For large syncs, extracting the microSD is faster and safer.

## Why it exists

The ecosystem is fragmented. `beets-alternatives` is powerful but
requires adopting beets as a library manager. DAP-specific GUIs
(FiiO Music Manager, DapSync) are platform-bound. Plain `rsync` knows
nothing about FAT32 filename limits, your DAP's codec matrix, or the
cache folders its firmware leaves behind.

`dapctl` is the pattern many audiophiles already run as ad-hoc shell
scripts, packaged as an honest, auditable, portable tool.

## Commands

```
dapctl                          # launch TUI (home screen)
dapctl sync <profile>           # sync to DAP (dry-run by default in mirror mode)
dapctl diff <profile>           # preview without touching the destination
dapctl diff <profile> --json    # machine-readable plan
dapctl scan                     # detect removable drives, identify DAPs
dapctl scan --json
dapctl profile list             # list DAP profiles + user sync profiles
dapctl profile show <dap-id>    # full DAP profile details
dapctl profile check <file>     # validate a sync profile TOML
dapctl profile delete <name>    # remove a sync profile (prompts for confirmation)
dapctl log                      # tail the structured log
dapctl export m3u <profile>     # generate M3U playlist for the DAP
dapctl export m3u <profile> -o playlist.m3u
dapctl audit <path>             # offline library health report (tags, covers, gaps)
dapctl audit <path> --json
dapctl cover fetch <path> --online   # download missing folder.jpg (opt-in)
dapctl cover embed <path>            # embed folder.jpg into track tags
```

Inside the TUI, press `m` from the profiles screen to open the audio
player. The player browses your source library, plays directly from
there or from a mounted DAP, and supports DSD via ffmpeg.

**Key bindings (player):** `space` play/pause · `n`/`p` next/prev ·
`j`/`k` navigate · `Enter` expand/play · `Tab` switch pane ·
`/` search · `←`/`→` seek · `+`/`-` volume · `L`/`D` toggle source ·
`i` toggle lyrics/queue · `r` cycle repeat · `s` shuffle · `t` sleep timer

Every TUI action has a non-interactive CLI equivalent (`--yes`,
`--dry-run`, `-v`) so it composes with scripts and cron.

## Platforms

Linux x86_64 / ARM64 · macOS Intel + Apple Silicon · Windows x86_64
(native and WSL).

## Building

```sh
cargo build --release
./target/release/dapctl --help
```

Requires Rust stable (≥ 1.80). No external dependencies beyond the
Rust toolchain — `ffmpeg` is optional and only needed for transcoding;
it is detected in PATH at runtime. If not found, transcoding entries
are skipped with a warning.

## DAP profiles

dapctl ships 7 builtin profiles:
- **FiiO M21** (`fiio-m21`) — ground-truth device of the author
- **FiiO M11** (`fiio-m11`)
- **Astell&Kern SR35** (`ak-sr35`)
- **HiBy R6** (`hiby-r6`)
- **Shanling M3 Ultra** (`shanling-m3ultra`)
- **iBasso DX320** (`ibasso-dx320`)
- **Generic** (`generic`) — conservative fallback for any DAP

See [`docs/DAP_PROFILE_SPEC.md`](docs/DAP_PROFILE_SPEC.md)
to add your device.

## Contributing

See [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md). The most
valuable contribution is a **DAP profile** for a device you own.

## What `dapctl` is not

- **Not a library manager.** The built-in player lets you browse and
  listen to your source library or verify what landed on the DAP after
  a sync. Your DAP remains your primary listening device.
- Not a tag editor (that's Picard or beets).
- Not a bidirectional sync tool (that's Syncthing).
- No GUI. Ever.
- **Offline by default.** No telemetry. `dapctl cover fetch` and
  `dapctl audit` make network calls only when you pass `--online`
  explicitly. SSH source (`ssh://host/path`) is opt-in and uses your
  existing `~/.ssh` config.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
