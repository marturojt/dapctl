# dapctl

> TUI/CLI sync tool for HiFi Digital Audio Players.
> `systemctl`-style subcommands. Rust + Ratatui. GPLv3.

`dapctl` is a terminal-first sync tool that knows about your DAP. It
compares your music library against the destination, shows a clear diff,
and copies only what's needed — respecting per-device quirks: filesystem
limits, supported codecs, cache folders to exclude.

## Status

**Active development — pre-0.1.** Core modules working:
`logging` · `config` · `dap` · `scan` · `diff`.
`transfer` (the actual sync) is next. See [`BACKLOG.md`](BACKLOG.md).

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
dapctl                        # launch TUI
dapctl sync <profile>         # sync to DAP (dry-run by default in mirror mode)
dapctl diff <profile>         # preview without touching the destination
dapctl diff <profile> --json  # machine-readable plan
dapctl scan                   # detect removable drives, identify DAPs
dapctl scan --json
dapctl profile list           # list DAP profiles + user sync profiles
dapctl profile show <dap-id>  # full DAP profile details
dapctl profile check <file>   # validate a sync profile TOML
dapctl log                    # tail the structured log
```

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

Requires Rust stable (≥ 1.75). No external dependencies beyond the
Rust toolchain — `ffmpeg` is only needed for transcoding (v0.2,
optional, detected in PATH at runtime).

## DAP profiles

dapctl ships builtin profiles for:
- **FiiO M21** (`fiio-m21`) — ground-truth device of the author
- **Generic** (`generic`) — conservative fallback for any DAP

Profiles for AK SR35 and HiBy R6 are stubs pending contributor
fixtures. See [`docs/DAP_PROFILE_SPEC.md`](docs/DAP_PROFILE_SPEC.md)
to add your device.

## Contributing

See [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md). The most
valuable contribution is a **DAP profile** for a device you own.

## What `dapctl` is not

- Not a music player.
- Not a tag editor (that's Picard or beets).
- Not a generic library manager.
- Not a bidirectional sync tool (that's Syncthing).
- No GUI. Ever.
- No telemetry. No network calls unless you configure an SSH source.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
