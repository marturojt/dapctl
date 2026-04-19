# dapctl

> TUI/CLI sync tool for HiFi Digital Audio Players.
> `systemctl`-style subcommands. Rust + Ratatui. GPLv3.

`dapctl` is a terminal-first sync tool that knows about your DAP. It
compares your music library against the microSD (or mounted player),
shows a clear diff, and copies only what's needed, respecting per-device
quirks: filesystem limits, supported codecs, cache folders to exclude.

## Status

**Pre-0.1.** Scaffolding in place; no working command yet. See
[`BACKLOG.md`](BACKLOG.md) for the roadmap, and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for module layout.

## Why it exists

The ecosystem is fragmented. `beets-alternatives` is powerful but
requires adopting beets. DAP-specific GUIs (FiiO Music Manager, DapSync)
are platform-bound. Plain `rsync` knows nothing about FAT32 filename
limits, your DAP's codec matrix, or the cache folders its firmware
leaves behind.

`dapctl` is the pattern many audiophiles already run as ad-hoc shell
scripts, packaged as an honest, auditable, portable tool.

## Planned commands

```
dapctl                 # launch TUI
dapctl sync <profile>  # execute a sync
dapctl diff <profile>  # preview without touching the destination
dapctl scan            # detect removable drives and identify DAPs
dapctl profile list    # list sync profiles
dapctl profile show <dap>
dapctl log             # tail the structured log
```

Every TUI action has a non-interactive CLI equivalent (`--yes`,
`--dry-run`, `--profile`) so it composes with scripts.

## Planned platforms

Linux x86_64 / ARM64, macOS (Intel + Apple Silicon), Windows x86_64
(native and WSL).

## Building (once source lands)

```sh
cargo build --release
./target/release/dapctl --help
```

## Contributing

See [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md). The most
valuable contribution is a **DAP profile** for a device you own —
see [`docs/DAP_PROFILE_SPEC.md`](docs/DAP_PROFILE_SPEC.md).

## What `dapctl` is not

- Not a music player.
- Not a tag editor (that's Picard or beets).
- Not a generic library manager.
- Not a bidirectional sync tool (that's Syncthing).
- No GUI. Ever.
- No telemetry. No network calls unless you configure an SSH source.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
