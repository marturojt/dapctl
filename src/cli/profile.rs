use clap::{Args as ClapArgs, Subcommand};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: ProfileCmd,
}

#[derive(Subcommand, Debug)]
pub enum ProfileCmd {
    /// List sync profiles found in the config directory.
    List,
    /// Show a resolved DAP profile (builtin or user override).
    Show {
        /// DAP profile id (e.g. `fiio-m21`). Run `list` to see options.
        id: String,
    },
    /// Validate a sync profile TOML against the schema.
    Check {
        /// Path to the sync profile TOML file.
        path: String,
    },
}

pub fn run(args: Args) -> anyhow::Result<()> {
    match args.cmd {
        ProfileCmd::List => cmd_list(),
        ProfileCmd::Show { id } => cmd_show(&id),
        ProfileCmd::Check { path } => cmd_check(&path),
    }
}

fn cmd_list() -> anyhow::Result<()> {
    println!("DAP PROFILES (builtin + user overrides)");
    println!("{}", "─".repeat(42));
    for id in crate::dap::list()? {
        println!("  {id}");
    }

    println!();
    println!("SYNC PROFILES  (~/.config/dapctl/profiles/)");
    println!("{}", "─".repeat(42));
    let profiles = crate::config::discover()?;
    if profiles.is_empty() {
        println!("  (none found — copy an example from the `examples/` directory)");
    } else {
        for (name, path) in &profiles {
            println!("  {name:<24}  {}", path.display());
        }
    }

    Ok(())
}

fn cmd_show(id: &str) -> anyhow::Result<()> {
    let dap = crate::dap::load(id)?;

    println!("DAP PROFILE  {}", dap.dap.id);
    println!("{}", "─".repeat(42));
    println!("  Name            {}", dap.dap.name);
    println!("  Vendor          {}", dap.dap.vendor);
    if let Some(fw) = &dap.dap.firmware_min {
        println!("  Firmware min    {fw}");
    }
    println!();
    println!("FILESYSTEM");
    println!("  Preferred       {}", dap.filesystem.preferred);
    println!("  Supported       {}", dap.filesystem.supported.join(", "));
    println!("  Max filename    {} bytes", dap.filesystem.max_filename_bytes);
    println!("  Case sensitive  {}", dap.filesystem.case_sensitive);
    println!();
    println!("CODECS");
    println!("  Lossless        {}", dap.codecs.lossless.join(", "));
    println!("  Lossy           {}", dap.codecs.lossy.join(", "));
    if !dap.codecs.dsd.is_empty() {
        println!("  DSD             {}", dap.codecs.dsd.join(", "));
    }
    println!(
        "  Max PCM         {} Hz / {} bit",
        dap.codecs.max_sample_rate_hz, dap.codecs.max_bit_depth
    );
    println!();
    println!("LAYOUT");
    println!("  Music root      {}", dap.layout.music_root);
    println!();
    println!("EXCLUDE GLOBS  ({} patterns)", dap.exclude.globs.len());
    for g in &dap.exclude.globs {
        println!("  {g}");
    }

    Ok(())
}

fn cmd_check(path: &str) -> anyhow::Result<()> {
    let profile = crate::config::load(std::path::Path::new(path))?;
    println!("OK  {}", profile.profile.name);
    println!("    source      → {}", profile.profile.source);
    println!("    destination → {}", profile.profile.destination);
    println!("    dap_profile → {}", profile.profile.dap_profile);
    println!("    mode        → {:?}", profile.profile.mode);

    // Also verify the referenced DAP profile resolves
    let dap = crate::dap::load(&profile.profile.dap_profile)?;
    println!("    DAP profile → {} ({})", dap.dap.name, dap.dap.id);

    // Build and validate globsets
    let resolved = crate::config::ResolvedProfile {
        sync: profile,
        dap,
    };
    let exc = resolved.build_exclude_set()?;
    let inc = resolved.build_include_set()?;
    println!(
        "    exclude set → {} patterns",
        resolved.all_exclude_globs().count()
    );
    println!(
        "    include set → {}",
        match &inc {
            Some(_) => format!("{} patterns", resolved.sync.filters.include_globs.len()),
            None => "all files (no include filter)".to_owned(),
        }
    );
    drop((exc, inc));

    Ok(())
}
