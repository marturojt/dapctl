use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: ExportCommand,
}

#[derive(Subcommand, Debug)]
pub enum ExportCommand {
    /// Generate an M3U playlist for a sync profile.
    ///
    /// Walks the source with the same filters that `dapctl sync` applies and
    /// outputs one path per file, prefixed with the DAP's `layout.music_root`.
    /// Place the result on the DAP to get a "full library" playlist.
    M3u(M3uArgs),
}

#[derive(ClapArgs, Debug)]
pub struct M3uArgs {
    /// Name of the sync profile.
    pub profile: String,

    /// Write the M3U to this file instead of stdout.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    match args.command {
        ExportCommand::M3u(a) => run_m3u(a),
    }
}

fn run_m3u(args: M3uArgs) -> anyhow::Result<()> {
    let resolved = crate::config::resolve(&args.profile)?;
    let source = camino::Utf8PathBuf::from(&resolved.sync.profile.source);

    let content = crate::export::m3u::generate(&resolved, &source)?;

    match args.output {
        Some(path) => {
            std::fs::write(&path, &content)?;
            eprintln!(
                "M3U written to {} ({} tracks)",
                path.display(),
                content.lines().filter(|l| !l.starts_with('#')).count(),
            );
        }
        None => print!("{content}"),
    }

    Ok(())
}
