//! Dual sink logging: human-readable to stderr+file, JSONL to a dedicated
//! per-run file. Schema versioned from day one.
//!
//! The JSONL schema is frozen at `v1` with fields:
//!   ts, level, run_id, event, path?, bytes?, err?

pub const JSONL_SCHEMA_VERSION: u32 = 1;

pub fn init() -> anyhow::Result<()> {
    Ok(())
}
