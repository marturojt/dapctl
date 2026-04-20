use clap::Args as ClapArgs;

use crate::scan::{fmt_bytes, Confidence};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Emit JSON instead of the human table.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    tracing::info!(event = "scan_start");
    let result = crate::scan::run_scan()?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let total = result.identified.len() + result.unidentified.len();
    println!(
        "REMOVABLE DRIVES  ({} found, {} identified as DAP)",
        total,
        result.identified.len()
    );
    println!("{}", "─".repeat(62));

    if result.identified.is_empty() && result.unidentified.is_empty() {
        println!("  (no removable drives detected)");
        return Ok(());
    }

    for d in &result.identified {
        let conf = match d.confidence {
            Confidence::Exact => "exact",
            Confidence::Heuristic => "heuristic",
            Confidence::Fallback => "fallback",
        };
        let size_info = match (d.mount.total_bytes, d.mount.free_bytes) {
            (Some(t), Some(f)) => format!(
                "  {free} free / {total}",
                free = fmt_bytes(f),
                total = fmt_bytes(t)
            ),
            _ => String::new(),
        };
        println!(
            "  {mount:<28}  [{dap_id}]  ({conf}){size}",
            mount = d.mount.mount_point,
            dap_id = d.dap_id,
            size = size_info,
        );
        if let Some(label) = &d.mount.label {
            println!("  {:<28}  label: {label}", "");
        }
        if let Some(fs) = &d.mount.filesystem {
            println!("  {:<28}  fs:    {fs}", "");
        }
    }

    for m in &result.unidentified {
        let size_info = match (m.total_bytes, m.free_bytes) {
            (Some(t), Some(f)) => format!("  {} free / {}", fmt_bytes(f), fmt_bytes(t)),
            _ => String::new(),
        };
        println!(
            "  {mount:<28}  [unknown]{size}",
            mount = m.mount_point,
            size = size_info,
        );
    }

    Ok(())
}
