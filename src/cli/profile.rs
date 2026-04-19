use clap::{Args as ClapArgs, Subcommand};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: ProfileCmd,
}

#[derive(Subcommand, Debug)]
pub enum ProfileCmd {
    /// List sync profiles found in the user config directory.
    List,
    /// Show the resolved DAP profile for a given id (builtin or override).
    Show { id: String },
    /// Validate a profile TOML against the schema.
    Check { path: String },
}

pub fn run(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("profile: not yet implemented (v0.1 milestone)")
}
