use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Name of the sync profile (as declared in its TOML).
    pub profile: String,

    /// Do not write to the destination; show what would happen.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("sync: not yet implemented (v0.1 milestone)")
}
