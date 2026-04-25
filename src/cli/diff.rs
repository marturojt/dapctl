use clap::Args as ClapArgs;

use crate::diff::EntryKind;
use crate::scan::fmt_bytes;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Name of the sync profile (as declared in its TOML `[profile] name`).
    pub profile: String,

    /// Emit JSON instead of the human table.
    #[arg(long)]
    pub json: bool,
}

/// Conservative write speed assumed for ETA estimate (30 MB/s to microSD).
const ESTIMATED_SPEED_BPS: u64 = 30 * 1024 * 1024;

pub fn run(args: Args) -> anyhow::Result<()> {
    let resolved = crate::config::resolve(&args.profile)?;

    let source = camino::Utf8PathBuf::from(&resolved.sync.profile.source);
    let destination = crate::scan::resolve_destination(&resolved.sync.profile.destination)?;

    tracing::info!(
        event = "diff_start",
        profile  = resolved.sync.profile.name,
        source   = %source,
        destination = %destination,
        mode     = ?resolved.sync.profile.mode,
    );

    let result = crate::diff::diff(&resolved, &source, &destination)?;
    let plan = &result.plan;

    if args.json {
        println!("{}", serde_json::to_string_pretty(plan)?);
        return Ok(());
    }

    // ── Summary ─────────────────────────────────────────────────────────────
    let new_b = plan.total_bytes(EntryKind::New);
    let mod_b = plan.total_bytes(EntryKind::Modified);
    let orp_b = plan.total_bytes(EntryKind::Orphan);

    println!(
        "DIFF  {}  →  {}",
        resolved.sync.profile.source, destination
    );
    println!(
        "      profile: {}  mode: {:?}  dap: {}",
        resolved.sync.profile.name,
        resolved.sync.profile.mode,
        resolved.dap.dap.id,
    );
    println!("{}", "─".repeat(62));

    println!(
        "  [+] {:>6}  new          {}",
        plan.count(EntryKind::New),
        fmt_bytes(new_b),
    );
    println!(
        "  [~] {:>6}  modified     {}",
        plan.count(EntryKind::Modified),
        fmt_bytes(mod_b),
    );
    println!(
        "  [-] {:>6}  orphans      {}",
        plan.count(EntryKind::Orphan),
        fmt_bytes(orp_b),
    );
    println!(
        "  [=] {:>6}  unchanged    {}",
        plan.count(EntryKind::Same),
        fmt_bytes(plan.total_bytes(EntryKind::Same)),
    );

    println!("{}", "─".repeat(62));

    let eta = plan.eta_secs(ESTIMATED_SPEED_BPS);
    let transfer_total = new_b + mod_b;
    println!(
        "  transfer: {}   ETA: {}",
        fmt_bytes(transfer_total),
        fmt_eta(eta),
    );

    // ── Entry list ───────────────────────────────────────────────────────────
    const MAX_SHOWN: usize = 40;
    let actionable: Vec<_> = plan
        .entries
        .iter()
        .filter(|e| e.kind != EntryKind::Same)
        .collect();

    if !actionable.is_empty() {
        println!();
        for entry in actionable.iter().take(MAX_SHOWN) {
            let tag = match entry.kind {
                EntryKind::New => "[+]",
                EntryKind::Modified => "[~]",
                EntryKind::Orphan => "[-]",
                EntryKind::Same => "[=]",
            };
            println!(
                "  {}  {:<50}  {}",
                tag,
                truncate(entry.path.as_ref(), 50),
                fmt_bytes(entry.size_bytes),
            );
        }
        if actionable.len() > MAX_SHOWN {
            println!(
                "  ... and {} more (use --json for full list)",
                actionable.len() - MAX_SHOWN
            );
        }
    }

    Ok(())
}

fn fmt_eta(secs: u64) -> String {
    if secs == 0 {
        return "< 1s".to_owned();
    }
    if secs < 60 {
        return format!("{secs}s");
    }
    let m = secs / 60;
    let s = secs % 60;
    if m < 60 {
        return format!("{m}m {s:02}s");
    }
    let h = m / 60;
    let m = m % 60;
    format!("{h}h {m:02}m")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    format!("…{}", &s[s.len().saturating_sub(max - 1)..])
}
