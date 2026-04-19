//! Compare source vs destination and produce a serialisable `Plan`.

pub mod compare;
pub mod plan;
pub mod walker;

pub use plan::{Entry, EntryKind, Plan};
