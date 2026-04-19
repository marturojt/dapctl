//! Execute a `Plan` with temp+fsync+rename, respecting parallelism and
//! dry-run. Emits `transfer::Event`s consumed by TUI and log.

use crate::diff::Plan;

pub struct Options {
    pub dry_run: bool,
    pub parallelism: usize,
}

pub fn execute(_plan: &Plan, _opts: &Options) -> anyhow::Result<()> {
    anyhow::bail!("transfer::executor: not yet implemented")
}
