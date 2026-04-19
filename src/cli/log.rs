use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Follow the active run instead of printing a snapshot.
    #[arg(short, long)]
    pub follow: bool,
    /// Filter by run id (ulid).
    #[arg(long)]
    pub run: Option<String>,
}

pub fn run(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("log: not yet implemented (v0.1 milestone)")
}
