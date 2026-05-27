# dapctl — Session Handoff

> Read this at the start of a new conversation to get full context.
> Last updated: 2026-05-26.

---

## What is this project

`dapctl` is a Rust + Ratatui TUI/CLI sync tool for HiFi Digital Audio Players (DAPs).
It syncs a music library to a microSD card by comparing source vs. destination,
showing a clear diff, and copying only what's needed — respecting per-device quirks:
FAT32/exFAT filename limits, supported codec matrices, firmware exclusion globs.

- **Repo:** `github.com/marturojt/dapctl`
- **Site:** `dapctl.com` (Next.js on Cloudflare Pages, repo: `marturojt/dapctl-site`)
- **Homebrew tap:** `marturojt/homebrew-tap` (local clone at `E:/Proyectos/homebrew-tap`)
- **Stack:** Rust stable ≥ 1.80, Ratatui 0.29, Clap 4, GPLv3
- **Author machine:** Windows 11, working directory `E:\Proyectos\dapctl`

---

## Current state: v1.0.0 released (2026-05-06)

All five milestones are complete. HEAD is `29ee82f` (style: apply cargo fmt) on `main`.

### What shipped in v1.0.0
- SSH source (`source = "ssh://user@host/path"`) — zero new Cargo deps, delegates auth to system `ssh`
- `dapctl cover embed <path>` — writes `folder.jpg` into FLAC/MP3/M4A/OGG/Opus tags via lofty
- `dapctl profile delete <name> [-y]` — CLI + TUI `D` key (two-press confirm)
- 5 new builtin DAP profiles → 7 total (fiio-m21, fiio-m11, ak-sr35, hiby-r6, shanling-m3ultra, ibasso-dx320, generic)
- `PathWarning` / `check_path_limits` — warns when filenames/paths exceed DAP firmware limits
- Typed error taxonomy (`src/error.rs`): `ConfigError`, `DapError`, `ScanError`; exit codes 2/3
- `NO_COLOR` env var support in `Theme::new()`
- Volume label detection on Linux (`lsblk`) and macOS (`diskutil`)
- Selective mode write-back via `toml_edit` (`x` key toggles album selection ◆/◇)
- Snapshot tests via `insta` — 54 tests total (`tests/snapshots.rs`)

### CI status
The `rustfmt` job failed on the v1.0.0 commit (`defb632`) — fixed in the next commit
`29ee82f` (style: apply cargo fmt). CI should be green on main now.

A **pre-commit hook** is configured in `.claude/settings.local.json` that auto-runs
`cargo fmt --all` and re-stages files before every `git commit` made by Claude Code,
so this should not happen again.

---

## Repository layout

```
src/
  cli/          clap subcommand dispatch
  tui/          ratatui app, views, theme
  config/       sync profile TOML (user-authored)
  dap/          DAP profile TOML (builtin + XDG override)
  scan/         removable drive enumeration + DAP identification
  diff/         walker, comparator, serialisable Plan, path-limit checks
  transfer/     executor (temp+rename), manifest (JSONL resume), verify
  transcode/    ffmpeg detection + engine + blake3-keyed cache
  export/       M3U playlist generation
  logging/      tracing dual sinks (human + JSONL v1)
  player/       rodio/symphonia engine, queue, decoder, library (SQLite),
                lyrics (.lrc), history/resume, gapless
  audit/        offline library health scanner + structured report
  cover/        MusicBrainz/CAA/iTunes fetch (reqwest) + tag embed (lofty)
  ssh/          SSH source adapter (system ssh binary, zero new deps)
  error.rs      typed thiserror enums; exit codes 2/3 in main.rs

profiles/       7 builtin DAP TOML profiles (embedded via include_dir!)
tests/          54 tests — integration, diff, checksum, tag filters, snapshots
packaging/
  scoop/dapctl.json     Scoop manifest skeleton (NOT yet in public bucket)
  aur/PKGBUILD          AUR PKGBUILD skeleton (NOT yet published)
docs/
  ARCHITECTURE.md       Module layout, data flow, SSH + player internals
  DAP_PROFILE_SPEC.md   Schema contract for DAP profiles
  CONTRIBUTING.md       PR guide, DAP profile contribution process
  NETWORK.md            cover fetch policy — opt-in --online, rate limits
  FILESYSTEM_NOTES.md   FAT32/exFAT/NTFS gotchas
  MTP_WORKFLOW.md       MTP workaround (jmtpfs / rclone + WinFsp)
```

---

## Open work (next session priorities)

### 1. Official distribution (highest impact)

| Package manager | Status | What's needed |
|-----------------|--------|---------------|
| **Homebrew core** | Not submitted | PR to `homebrew/homebrew-core`. Requires: ≥30 days public, tests pass, formula meets [Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae). The tap (`marturojt/tap`) already works as a bridge. |
| **Scoop main bucket** | Skeleton ready at `packaging/scoop/dapctl.json` | Fork `ScoopInstaller/Main`, add the manifest, open PR |
| **AUR** | Skeleton ready at `packaging/aur/PKGBUILD` | Needs AUR account + `makepkg -si` test on Arch, then `git push` to AUR |
| **crates.io** | Not published | `cargo publish` — confirm namespace not taken first |

### 2. AcoustID duplicate detection (optional feature)

Detect duplicate tracks by audio fingerprint, not just filename.
- External dependency: `chromaprint` CLI (`fpcalc`) in PATH — same pattern as ffmpeg.
- Workflow: `dapctl audit --dupes <path>` — fingerprint all tracks, group by AcoustID lookup.
- Relevant crate: `acoustid` or raw HTTP to `api.acoustid.org`.
- No new Cargo deps if we shell out to `fpcalc` (same zero-dep pattern as SSH).

### 3. Beets integration as filter source

Allow `source = "beets:query"` or `[filters] beets_query = "genre:Jazz"` so users
who already use beets as a library manager can drive which tracks sync.
- Shells out to `beets ls -f '$path' <query>` and uses the result as an include list.
- No new Cargo deps.

### 4. Manifest resume integration test

A test that kills the executor mid-sync and verifies a re-run skips `Done` entries
and retries `InProgress`/`pending`. This is the only item left from the original 11
MVP requirements that has no test coverage.

---

## Key files to read for context

| File | Why |
|------|-----|
| `src/ssh/mod.rs` | Full SSH source implementation (SshUri, SshSession, walk, download) |
| `src/error.rs` | ConfigError / DapError / ScanError enums |
| `src/diff/mod.rs` | Where SSH source branches away from local walker |
| `src/transfer/executor.rs` | Three-way copy branch (SSH / transcode / local) |
| `src/tui/app.rs` | App state, delete_confirm field, delete_current_profile() |
| `src/tui/mod.rs` | Key handler — D key two-press confirm, SSH session wiring |
| `profiles/fiio-m21.toml` | Canonical DAP profile example |
| `examples/sync-fiio-m21-flac.toml` | Canonical sync profile example |
| `docs/ARCHITECTURE.md` | Full module map + data flow |

---

## Commands cheatsheet (all subcommands as of v1.0.0)

```
dapctl                                  # TUI home screen
dapctl sync <profile> [--yes] [--dry-run]
dapctl diff <profile> [--json]
dapctl scan [--json]
dapctl profile list
dapctl profile show <dap-id>
dapctl profile check <file>
dapctl profile delete <name> [-y]
dapctl log
dapctl export m3u <profile> [-o file]
dapctl audit <path> [--json] [--min-severity high|med|low] [--limit N]
dapctl cover fetch <path> --online
dapctl cover embed <path> [--overwrite]
```

TUI player key bindings: `space` play/pause · `n`/`p` next/prev · `j`/`k` nav ·
`Enter` play · `Tab` switch pane · `/` search · `←`/`→` seek · `+`/`-` volume ·
`L`/`D` toggle source (library/DAP) · `i` lyrics/queue toggle · `r` repeat ·
`s` shuffle · `t` sleep timer · `D` delete profile (two-press)

---

## Dev workflow

```sh
cargo build                   # debug build
cargo test                    # 54 tests
cargo fmt --all -- --check    # CI gate (auto-fixed by pre-commit hook)
cargo clippy -- -D warnings   # CI gate
cargo run -- scan             # quick smoke test
```

CI runs on push to `main` and on any tag. Release workflow triggers on `v*` tags
and produces binaries + SHA256SUMS for Linux x86_64/aarch64, macOS universal,
Windows x86_64.

---

## Infrastructure state

| Thing | Location | Notes |
|-------|----------|-------|
| Main repo | `E:\Proyectos\dapctl` | on `main`, clean |
| Site repo | `E:\Proyectos\dapctl-site` | on `main`, shows v1.0.0 |
| Homebrew tap | `E:\Proyectos\homebrew-tap` | formula updated to v1.0.0 |
| Pre-commit hook | `.claude/settings.local.json` | Claude Code only, not committed |
| GH Releases | github.com/marturojt/dapctl/releases | v1.0.0 published with all binaries |
