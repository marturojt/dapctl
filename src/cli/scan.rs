use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Emit JSON instead of the human table.
    #[arg(long)]
    pub json: bool,
}

pub fn run(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("scan: not yet implemented (v0.1 milestone)")
}
