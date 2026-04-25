use std::path::PathBuf;

use clap::{Parser, Subcommand};
use ulid::Ulid;

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

    /// Write human-readable log to this file in addition to stderr.
    #[arg(long, global = true, value_name = "PATH")]
    log_file: Option<PathBuf>,

    /// Increase verbosity (default: INFO).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

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

/// Parse argv, initialise logging, and dispatch.
/// When no subcommand is given, launch the TUI.
pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let verbosity = match cli.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    let run_id = Ulid::new();
    let jsonl_dir = crate::logging::default_jsonl_dir()?;
    crate::logging::init(crate::logging::InitOpts {
        run_id,
        human_log_file: cli.log_file,
        jsonl_dir,
        verbosity,
        tui_mode: cli.command.is_none(),
    })?;

    let result = match cli.command {
        None => crate::tui::run(),
        Some(Command::Sync(a)) => sync::run(a, cli.yes),
        Some(Command::Diff(a)) => diff::run(a),
        Some(Command::Scan(a)) => scan::run(a),
        Some(Command::Profile(a)) => profile::run(a),
        Some(Command::Log(a)) => log::run(a),
    };

    crate::logging::finish(result.is_ok());
    result
}
