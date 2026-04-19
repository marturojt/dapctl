//! Top-level app state machine: which view is active, shared state, event loop.

#[derive(Debug, Clone, Copy)]
pub enum View {
    Profiles,
    Diff,
    Progress,
    Log,
}
