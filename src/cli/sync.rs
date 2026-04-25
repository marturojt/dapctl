use clap::Args as ClapArgs;

use crate::config::Mode;
use crate::diff::EntryKind;
use crate::scan::fmt_bytes;
use crate::transfer::executor::{Options, SyncMode};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Name of the sync profile (as declared in its TOML `[profile] name`).
    pub profile: String,

    /// Simulate without writing; overrides the profile's `dry_run_default`.
    #[arg(long)]
    pub dry_run: bool,
}

/// Conservative write speed used for ETA (30 MB/s to microSD).
const ESTIMATED_SPEED_BPS: u64 = 30 * 1024 * 1024;

pub fn run(args: Args, yes: bool) -> anyhow::Result<()> {
    let resolved = crate::config::resolve(&args.profile)?;

    let source = camino::Utf8PathBuf::from(&resolved.sync.profile.source);
    let destination = crate::scan::resolve_destination(&resolved.sync.profile.destination)?;

    // Determine effective dry-run: explicit flag > --yes > profile default.
    let dry_run = if args.dry_run {
        true
    } else if yes {
        false
    } else {
        resolved.sync.transfer.dry_run_default
    };

    let mode = match resolved.sync.profile.mode {
        Mode::Mirror => SyncMode::Mirror,
        Mode::Additive | Mode::Selective => SyncMode::Additive,
    };

    tracing::info!(
        event = "sync_start",
        profile  = resolved.sync.profile.name,
        source   = %source,
        destination = %destination,
        mode     = ?resolved.sync.profile.mode,
        dry_run,
    );

    // Repair destination mtimes for files copied without mtime preservation.
    let repaired = crate::transfer::repair_dest_mtimes(&source, &destination);
    if repaired > 0 {
        eprintln!("  repaired mtimes for {repaired} file(s)");
    }

    // ── Diff ──────────────────────────────────────────────────────────────
    let result = crate::diff::diff(&resolved, &source, &destination)?;
    let plan = &result.plan;

    let new_b = plan.total_bytes(EntryKind::New);
    let mod_b = plan.total_bytes(EntryKind::Modified);
    let orp_b = plan.total_bytes(EntryKind::Orphan);
    let transfer_total = new_b + mod_b;
    let eta = plan.eta_secs(ESTIMATED_SPEED_BPS);

    // ── Summary ───────────────────────────────────────────────────────────
    println!(
        "SYNC  {}  →  {}",
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
        "  [+] {:>6}  new        {}",
        plan.count(EntryKind::New),
        fmt_bytes(new_b),
    );
    println!(
        "  [~] {:>6}  modified   {}",
        plan.count(EntryKind::Modified),
        fmt_bytes(mod_b),
    );
    println!(
        "  [-] {:>6}  orphans    {}",
        plan.count(EntryKind::Orphan),
        fmt_bytes(orp_b),
    );
    println!(
        "  [=] {:>6}  unchanged  {}",
        plan.count(EntryKind::Same),
        fmt_bytes(plan.total_bytes(EntryKind::Same)),
    );
    println!("{}", "─".repeat(62));
    println!(
        "  transfer: {}   ETA: {}",
        fmt_bytes(transfer_total),
        fmt_eta(eta),
    );

    // Nothing to do?
    if transfer_total == 0 && plan.count(EntryKind::Orphan) == 0 {
        println!("\nNothing to sync.");
        return Ok(());
    }

    // ── Confirm mirror deletions ───────────────────────────────────────────
    let has_deletions =
        matches!(mode, SyncMode::Mirror) && plan.count(EntryKind::Orphan) > 0;

    if dry_run {
        println!();
        if resolved.sync.transfer.dry_run_default && !yes {
            println!(
                "  (dry run — pass --yes to execute{})",
                if has_deletions {
                    ", including deletion of orphans"
                } else {
                    ""
                }
            );
        } else {
            println!("  (dry run)");
        }
        return Ok(());
    }

    if has_deletions && !yes {
        anyhow::bail!(
            "{} orphan(s) would be deleted in mirror mode. \
             Pass --yes to confirm or use mode=additive to skip deletions.",
            plan.count(EntryKind::Orphan)
        );
    }

    println!();

    // ── Execute ───────────────────────────────────────────────────────────
    let manifest_dir = manifest_dir()?;
    let run_id = crate::logging::current_run_id();

    let opts = Options {
        dry_run: false,
        mode,
        verify: resolved.sync.transfer.verify,
        run_id,
        manifest_dir,
    };

    let stats = crate::transfer::execute(plan, &source, &destination, &opts)?;

    // ── Result summary ────────────────────────────────────────────────────
    println!();
    println!(
        "Sync complete: {} copied, {} deleted, {} failed  ({})",
        stats.copied,
        stats.deleted,
        stats.failed,
        fmt_eta(stats.elapsed_secs as u64),
    );

    if stats.failed > 0 {
        anyhow::bail!("{} file(s) failed to transfer", stats.failed);
    }

    tracing::info!(
        event = "sync_done",
        copied = stats.copied,
        deleted = stats.deleted,
        failed = stats.failed,
        bytes = stats.bytes_written,
    );

    Ok(())
}

fn manifest_dir() -> anyhow::Result<camino::Utf8PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")
        .ok_or_else(|| anyhow::anyhow!("cannot determine data directory"))?;
    let path = dirs.data_local_dir().join("runs");
    camino::Utf8PathBuf::from_path_buf(path)
        .map_err(|p| anyhow::anyhow!("non-UTF-8 data dir: {}", p.display()))
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
