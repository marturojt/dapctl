//! User-facing sync profile: parse and validate the TOML that the user
//! writes in `$XDG_CONFIG_HOME/dapctl/profiles/*.toml`.

pub mod schema;

pub use schema::{Filters, Mode, SyncProfile, Transcode, Transfer, Verify};
