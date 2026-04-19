//! DAP catalogue: builtin profiles embedded at compile time, plus loader
//! for per-user overrides under `$XDG_CONFIG_HOME/dapctl/profiles/`.

pub mod builtin;
pub mod schema;

pub use schema::{Codecs, DapHeader, DapProfile, Exclude, Filesystem, Layout, Quirks};
