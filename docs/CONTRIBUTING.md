# Contributing

`dapctl` is a personal project open to contributions from a small
circle of audiophile friends. The bar for quality is high; the bar for
ceremony is low.

## The most valuable contribution: a DAP profile

If you own a DAP that lacks a profile, contribute one.

1. Read [`DAP_PROFILE_SPEC.md`](DAP_PROFILE_SPEC.md).
2. Copy `profiles/generic.toml` to `profiles/<vendor-model>.toml`.
3. Fill every field. Cite sources for codec / sample-rate claims.
4. Add a fixture under `tests/fixtures/profiles/<id>/` that exercises
   max filename length, a file at the highest supported sample rate,
   and one sample of each listed codec.
5. Open a PR titled `dap: add <vendor> <model>`.

## Code contributions

- Rust `stable` (see `rust-version` in `Cargo.toml`).
- `cargo fmt` and `cargo clippy -- -D warnings` must pass locally.
- Tests should not rely on network or external binaries unless
  explicitly gated (`#[cfg(feature = "ffmpeg-tests")]` etc.).
- Commit style: conventional commits — `feat:`, `fix:`, `dap:`,
  `docs:`, `refactor:`, `test:`, `ci:`, `build:`.

## What won't be merged

- Features outside scope (see README *What `dapctl` is not*).
- Platform-specific bug workarounds without a test reproducing the bug.
- DAP profiles without sources or fixtures.
- Additions that pull in heavyweight dependencies for marginal value.

## Discussions

Open an issue before a large PR. A 50-line bug fix doesn't need one;
a new subcommand or a change to the DAP profile schema does.
