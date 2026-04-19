use clap::Args as ClapArgs;

#[derive(ClapArgs, Debug)]
pub struct Args {
    pub profile: String,
}

pub fn run(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("diff: not yet implemented (v0.1 milestone)")
}
