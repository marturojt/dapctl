//! `dapctl cover fetch|embed` subcommands.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: CoverCommand,
}

#[derive(Subcommand, Debug)]
pub enum CoverCommand {
    /// Download missing cover art (folder.jpg) for albums in a library path.
    Fetch(FetchArgs),
    /// Embed folder.jpg into audio file tags for albums that have it on disk.
    Embed(EmbedArgs),
}

#[derive(ClapArgs, Debug)]
pub struct FetchArgs {
    /// Root path of the library to scan (defaults to the first sync profile's source).
    path: Option<PathBuf>,

    /// Enable network requests. Required — cover fetch makes HTTP calls to
    /// MusicBrainz and iTunes. See docs/NETWORK.md for the full policy.
    #[arg(long)]
    online: bool,

    /// Print results as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(ClapArgs, Debug)]
pub struct EmbedArgs {
    /// Root path of the library to scan (defaults to the first sync profile's source).
    path: Option<PathBuf>,

    /// Replace existing embedded cover art (default: skip files that already have art).
    #[arg(long)]
    overwrite: bool,

    /// Print results as JSON.
    #[arg(long)]
    json: bool,
}

pub fn run(args: self::Args) -> anyhow::Result<()> {
    match args.command {
        CoverCommand::Fetch(a) => run_fetch(a),
        CoverCommand::Embed(a) => run_embed(a),
    }
}

fn run_fetch(args: FetchArgs) -> anyhow::Result<()> {
    if !args.online {
        eprintln!("cover fetch requires --online.");
        eprintln!("See docs/NETWORK.md for the network policy, rate limits, and opt-in rationale.");
        std::process::exit(2);
    }

    let path = match args.path {
        Some(p) => p,
        None => resolve_library_path()?,
    };

    println!(
        "Scanning {} for albums without cover art\u{2026}",
        path.display()
    );

    let opts = crate::cover::FetchOptions { path: path.clone() };
    let stats = crate::cover::fetch(&opts, |msg| println!("{msg}"))?;

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "albums_scanned": stats.albums_scanned,
                "already_have":   stats.already_have,
                "fetched":        stats.fetched,
                "not_found":      stats.not_found,
                "errors":         stats.errors,
            })
        );
    } else {
        println!();
        println!("  {} albums scanned", stats.albums_scanned);
        println!("  {} already had cover art", stats.already_have);
        println!("  {} covers fetched", stats.fetched);
        println!("  {} not found", stats.not_found);
        if stats.errors > 0 {
            println!("  {} errors", stats.errors);
        }
    }

    Ok(())
}

fn run_embed(args: EmbedArgs) -> anyhow::Result<()> {
    let path = match args.path {
        Some(p) => p,
        None => resolve_library_path()?,
    };

    println!("Embedding cover art in {}\u{2026}", path.display());

    let opts = crate::cover::EmbedOptions {
        path,
        overwrite: args.overwrite,
    };
    let stats = crate::cover::embed(&opts, |msg| println!("{msg}"))?;

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "albums_scanned":         stats.albums_scanned,
                "files_embedded":         stats.files_embedded,
                "files_skipped_has_art":  stats.files_skipped_has_art,
                "files_skipped_no_folder": stats.files_skipped_no_folder,
                "files_skipped_format":   stats.files_skipped_format,
                "errors":                 stats.errors,
            })
        );
    } else {
        println!();
        println!("  {} albums scanned", stats.albums_scanned);
        println!("  {} files embedded", stats.files_embedded);
        if stats.files_skipped_has_art > 0 {
            println!(
                "  {} already had embedded art (use --overwrite to replace)",
                stats.files_skipped_has_art
            );
        }
        if stats.files_skipped_no_folder > 0 {
            println!(
                "  {} skipped \u{2014} no cover file in album dir",
                stats.files_skipped_no_folder
            );
        }
        if stats.files_skipped_format > 0 {
            println!(
                "  {} skipped \u{2014} format doesn't support tag embedding",
                stats.files_skipped_format
            );
        }
        if stats.errors > 0 {
            println!("  {} errors", stats.errors);
        }
    }

    Ok(())
}

fn resolve_library_path() -> anyhow::Result<PathBuf> {
    let discovered = crate::config::discover()?;
    for (_, path) in discovered {
        if let Ok(profile) = crate::config::load(&path) {
            let src = std::path::PathBuf::from(&profile.profile.source);
            if src.exists() {
                return Ok(src);
            }
        }
    }
    anyhow::bail!("no path specified and no sync profiles found — pass a path explicitly")
}
