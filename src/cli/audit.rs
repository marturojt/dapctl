use std::path::PathBuf;

use clap::Args as ClapArgs;

use crate::audit::{AuditReport, Severity};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Path to the music library to audit (defaults to the source of the
    /// first available sync profile if omitted).
    pub path: Option<PathBuf>,

    /// Emit JSON instead of the human table.
    #[arg(long)]
    pub json: bool,

    /// Only report issues at or above this severity (high | med | low).
    #[arg(long, value_name = "LEVEL", default_value = "low")]
    pub min_severity: SeverityArg,

    /// Limit output to the first N albums.
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

/// Thin newtype so clap can parse "high" / "med" / "low".
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SeverityArg {
    High,
    Med,
    Low,
}

impl From<&SeverityArg> for Severity {
    fn from(a: &SeverityArg) -> Self {
        match a {
            SeverityArg::High => Severity::High,
            SeverityArg::Med => Severity::Medium,
            SeverityArg::Low => Severity::Low,
        }
    }
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let path = resolve_library_path(args.path)?;
    let min_sev = Severity::from(&args.min_severity);

    eprintln!("Scanning {} …", path.display());
    let report = crate::audit::scan(&path)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    print_human(&report, min_sev, args.limit);
    Ok(())
}

fn resolve_library_path(arg: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(p) = arg {
        return Ok(p);
    }
    // Fall back to the source of the first loadable sync profile.
    let profiles = crate::config::discover()?;
    let Some((_, path)) = profiles.into_iter().next() else {
        anyhow::bail!(
            "No library path given and no sync profile found; \
             pass a path: dapctl audit /path/to/music"
        );
    };
    let profile = crate::config::load(&path)?;
    Ok(PathBuf::from(profile.profile.source))
}

fn print_human(report: &AuditReport, min_sev: Severity, limit: Option<usize>) {
    println!("AUDIT  {}", report.library.display());
    println!(
        "       {} tracks · {} albums · {} with issues",
        report.tracks_scanned, report.albums_scanned, report.albums_with_issues,
    );
    println!("{}", "─".repeat(70));

    let filtered: Vec<_> = report
        .albums
        .iter()
        .filter(|a| a.max_severity().is_some_and(|s| s <= min_sev))
        .collect();

    let shown = match limit {
        Some(n) => &filtered[..filtered.len().min(n)],
        None => &filtered[..],
    };

    for album in shown {
        for ai in &album.issues {
            if ai.severity > min_sev {
                continue;
            }
            println!(
                "  {}  {:<42}  {}",
                ai.severity,
                truncate(&album.display, 42),
                ai.issue.description(),
            );
        }
    }

    if let (Some(_), true) = (limit, filtered.len() > shown.len()) {
        println!(
            "  … {} more albums (remove --limit to see all)",
            filtered.len() - shown.len()
        );
    }

    println!("{}", "─".repeat(70));
    println!(
        "  {} issues  ({} high · {} medium · {} low)",
        report.issues_total, report.high, report.medium, report.low,
    );

    if report.high > 0 {
        println!();
        println!("  Tip: run untagged tracks through MusicBrainz Picard");
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    format!("…{}", &s[s.len().saturating_sub(max - 1)..])
}
