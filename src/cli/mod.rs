//! Command-line entry point and subcommand dispatch.

use clap::{Parser, Subcommand};

pub mod diff;
pub mod log;
pub mod profile;
pub mod scan;
pub mod sync;

#[derive(Parser, Debug)]
#[command(
    name = "dapctl",
    version,
    about = "TUI/CLI sync for HiFi Digital Audio Players",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Write logs to this file in addition to stderr.
    #[arg(long, global = true, value_name = "PATH")]
    log_file: Option<String>,

    /// Assume yes to all prompts (required for destructive ops without a TTY).
    #[arg(short = 'y', long, global = true)]
    yes: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Synchronise a profile to its DAP.
    Sync(sync::Args),
    /// Show what a sync would do, without touching the destination.
    Diff(diff::Args),
    /// Detect removable drives and identify DAPs.
    Scan(scan::Args),
    /// Manage sync profiles and inspect DAP profiles.
    Profile(profile::Args),
    /// Tail or query the structured log.
    Log(log::Args),
}

/// Parse argv and dispatch. When no subcommand is given, launch the TUI.
pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => crate::tui::run(),
        Some(Command::Sync(a)) => sync::run(a),
        Some(Command::Diff(a)) => diff::run(a),
        Some(Command::Scan(a)) => scan::run(a),
        Some(Command::Profile(a)) => profile::run(a),
        Some(Command::Log(a)) => log::run(a),
    }
}
