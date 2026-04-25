//! Top-level app state machine: which view is active, shared state, event loop.

use camino::Utf8PathBuf;

use crate::config::{Mode, SyncProfile};
use crate::scan::ScanResult;
use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Profiles,
    Diff,
    Progress,
    Log,
}

/// State of the diff computation for the diff view.
pub enum DiffState {
    Idle,
    Loading,
    Ready {
        result: Box<crate::diff::DiffResult>,
        source: Utf8PathBuf,
        destination: Utf8PathBuf,
        profile_name: String,
        dap_id: String,
        mode: Mode,
    },
    Error(String),
}

/// Which entry kinds to show in the diff entry list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFilter {
    All,
    New,
    Modified,
    Orphan,
    Same,
}

impl EntryFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::New => "NEW [+]",
            Self::Modified => "MODIFIED [~]",
            Self::Orphan => "ORPHAN [-]",
            Self::Same => "SAME [=]",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::New,
            Self::New => Self::Modified,
            Self::Modified => Self::Orphan,
            Self::Orphan => Self::Same,
            Self::Same => Self::All,
        }
    }

    pub fn matches(self, kind: crate::diff::EntryKind) -> bool {
        use crate::diff::EntryKind;
        match self {
            Self::All => true,
            Self::New => kind == EntryKind::New,
            Self::Modified => kind == EntryKind::Modified,
            Self::Orphan => kind == EntryKind::Orphan,
            Self::Same => kind == EntryKind::Same,
        }
    }
}

pub struct App {
    pub view: View,
    pub theme: Theme,
    /// Loaded sync profiles: (display-name, profile).
    pub profiles: Vec<(String, SyncProfile)>,
    pub scan: ScanResult,
    /// Selected index in the profiles list.
    pub profile_idx: usize,
    pub should_quit: bool,
    /// Transient status message shown in the footer.
    pub flash: Option<String>,
    pub flash_ticks: u8,
    /// Set to true when the user confirms sync from the diff view.
    pub pending_sync: bool,
    /// True while waiting for a second `y` confirmation (mirror + orphans).
    pub confirm_sync: bool,

    // ── Diff view state ───────────────────────────────────────────────────
    pub diff_state: DiffState,
    pub diff_entry_idx: usize,
    pub diff_entry_filter: EntryFilter,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let discovered = crate::config::discover()?;
        let mut profiles = Vec::with_capacity(discovered.len());
        for (name, path) in discovered {
            match crate::config::load(&path) {
                Ok(p) => profiles.push((name, p)),
                Err(e) => tracing::warn!(profile = %path.display(), err = %e, "skipping profile"),
            }
        }

        let scan = crate::scan::run_scan().unwrap_or_else(|e| {
            tracing::warn!(err = %e, "scan failed");
            ScanResult { identified: vec![], unidentified: vec![] }
        });

        Ok(Self {
            view: View::Profiles,
            theme: Theme::default(),
            profiles,
            scan,
            profile_idx: 0,
            should_quit: false,
            flash: None,
            flash_ticks: 0,
            pending_sync: false,
            confirm_sync: false,
            diff_state: DiffState::Idle,
            diff_entry_idx: 0,
            diff_entry_filter: EntryFilter::All,
        })
    }

    pub fn selected_profile(&self) -> Option<&SyncProfile> {
        self.profiles.get(self.profile_idx).map(|(_, p)| p)
    }

    // ── Profiles view ─────────────────────────────────────────────────────

    pub fn move_up(&mut self) {
        if self.profile_idx > 0 {
            self.profile_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.profiles.is_empty() && self.profile_idx + 1 < self.profiles.len() {
            self.profile_idx += 1;
        }
    }

    pub fn refresh_scan(&mut self) {
        if let Ok(s) = crate::scan::run_scan() {
            self.scan = s;
            self.set_flash("scan refreshed");
        }
    }

    // ── Diff view ─────────────────────────────────────────────────────────

    pub fn enter_diff(&mut self) {
        self.diff_state = DiffState::Loading;
        self.diff_entry_idx = 0;
        self.diff_entry_filter = EntryFilter::All;
        self.view = View::Diff;
    }

    pub fn move_diff_up(&mut self) {
        self.diff_entry_idx = self.diff_entry_idx.saturating_sub(1);
    }

    pub fn move_diff_down(&mut self) {
        self.diff_entry_idx = self.diff_entry_idx.saturating_add(1);
    }

    pub fn cycle_diff_filter(&mut self) {
        self.diff_entry_filter = self.diff_entry_filter.next();
        self.diff_entry_idx = 0;
    }

    // ── Shared helpers ────────────────────────────────────────────────────

    pub fn set_flash(&mut self, msg: impl Into<String>) {
        self.flash = Some(msg.into());
        self.flash_ticks = 12;
    }

    pub fn tick_flash(&mut self) {
        if self.flash.is_some() {
            self.flash_ticks = self.flash_ticks.saturating_sub(1);
            if self.flash_ticks == 0 {
                self.flash = None;
            }
        }
    }
}
