//! Top-level app state machine: which view is active, shared state, event loop.

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use camino::Utf8PathBuf;

use crate::config::{Mode, SyncProfile};
use crate::scan::ScanResult;
use crate::transfer::{ProgressEvent, Stats};
use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Profiles,
    Diff,
    Progress,
    Log,
    NewProfile,
}

/// State of the diff computation.
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

/// Which entry kinds to show in the diff list.
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

// ── New profile wizard state ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Name,
    Source,
    Destination,
    DapProfile,
    Mode,
    Confirm,
}

impl WizardStep {
    pub fn number(self) -> usize {
        match self {
            Self::Name => 1,
            Self::Source => 2,
            Self::Destination => 3,
            Self::DapProfile => 4,
            Self::Mode => 5,
            Self::Confirm => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "profile name",
            Self::Source => "source path",
            Self::Destination => "destination",
            Self::DapProfile => "DAP profile",
            Self::Mode => "sync mode",
            Self::Confirm => "confirm",
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Name => None,
            Self::Source => Some(Self::Name),
            Self::Destination => Some(Self::Source),
            Self::DapProfile => Some(Self::Destination),
            Self::Mode => Some(Self::DapProfile),
            Self::Confirm => Some(Self::Mode),
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Name => Some(Self::Source),
            Self::Source => Some(Self::Destination),
            Self::Destination => Some(Self::DapProfile),
            Self::DapProfile => Some(Self::Mode),
            Self::Mode => Some(Self::Confirm),
            Self::Confirm => None,
        }
    }
}

pub struct NewProfileState {
    pub step: WizardStep,
    pub name: tui_input::Input,
    pub source: tui_input::Input,
    /// Index into [identified DAPs..., manual]. Last item is always "Manual".
    pub dest_choice: usize,
    /// Active when dest_choice == identified.len() (manual).
    pub dest_manual: tui_input::Input,
    /// Whether the destination text-input is focused (manual mode).
    pub dest_manual_active: bool,
    pub dap_choice: usize,
    pub dap_ids: Vec<String>,
    pub mode_choice: usize,
    pub error: Option<String>,
}

impl NewProfileState {
    pub fn new(dap_ids: Vec<String>) -> Self {
        Self {
            step: WizardStep::Name,
            name: tui_input::Input::default(),
            source: tui_input::Input::default(),
            dest_choice: 0,
            dest_manual: tui_input::Input::default(),
            dest_manual_active: false,
            dap_choice: 0,
            dap_ids,
            mode_choice: 0,
            error: None,
        }
    }

    /// Resolved destination string for the current choice + scan.
    pub fn destination(&self, scan: &crate::scan::ScanResult) -> String {
        let manual_idx = scan.identified.len();
        if self.dest_choice == manual_idx {
            self.dest_manual.value().to_owned()
        } else if let Some(id) = scan.identified.get(self.dest_choice) {
            format!("auto:{}", id.dap_id)
        } else {
            String::new()
        }
    }

    pub fn selected_dap(&self) -> &str {
        self.dap_ids.get(self.dap_choice).map(String::as_str).unwrap_or("generic")
    }

    pub fn selected_mode(&self) -> &'static str {
        if self.mode_choice == 0 { "additive" } else { "mirror" }
    }
}

// ── Progress view state ───────────────────────────────────────────────────────

const MAX_RECENT: usize = 200;

pub struct ProgressState {
    pub profile_name: String,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub current_file: String,
    pub current_file_bytes: u64,
    pub current_file_done: u64,
    pub copied: usize,
    pub deleted: usize,
    pub failed: usize,
    pub recent: VecDeque<RecentLine>,
    pub finished: bool,
    pub finish_stats: Option<Stats>,
    started: Instant,
}

pub struct RecentLine {
    pub icon: &'static str,
    pub path: String,
    pub ok: bool,
}

impl ProgressState {
    pub fn new(profile_name: String, total_bytes: u64) -> Self {
        Self {
            profile_name,
            total_bytes,
            done_bytes: 0,
            current_file: String::new(),
            current_file_bytes: 0,
            current_file_done: 0,
            copied: 0,
            deleted: 0,
            failed: 0,
            recent: VecDeque::with_capacity(MAX_RECENT + 1),
            finished: false,
            finish_stats: None,
            started: Instant::now(),
        }
    }

    pub fn speed_bps(&self) -> f64 {
        let secs = self.started.elapsed().as_secs_f64();
        if secs < 0.5 {
            0.0
        } else {
            self.done_bytes as f64 / secs
        }
    }

    pub fn eta_secs(&self) -> u64 {
        let speed = self.speed_bps();
        if speed < 1.0 || self.total_bytes == 0 {
            return 0;
        }
        let remaining = self.total_bytes.saturating_sub(self.done_bytes);
        (remaining as f64 / speed) as u64
    }

    pub fn handle_event(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::FileStart { path, bytes } => {
                self.current_file = path;
                self.current_file_bytes = bytes;
                self.current_file_done = 0;
            }
            ProgressEvent::FileProgress { bytes } => {
                self.current_file_done += bytes;
                self.done_bytes += bytes;
            }
            ProgressEvent::FileDone { path, bytes: _ } => {
                self.copied += 1;
                self.current_file.clear();
                self.push_recent(RecentLine { icon: "[+]", path, ok: true });
            }
            ProgressEvent::FileFail { path, err } => {
                self.failed += 1;
                self.current_file.clear();
                self.push_recent(RecentLine {
                    icon: "[!]",
                    path: format!("{path}  ({err})"),
                    ok: false,
                });
            }
            ProgressEvent::DeleteDone { path } => {
                self.deleted += 1;
                self.push_recent(RecentLine { icon: "[-]", path, ok: true });
            }
            ProgressEvent::Finish { stats } => {
                self.finished = true;
                self.finish_stats = Some(stats);
                self.current_file.clear();
            }
        }
    }

    fn push_recent(&mut self, line: RecentLine) {
        if self.recent.len() >= MAX_RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(line);
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub view: View,
    pub theme: Theme,
    pub profiles: Vec<(String, SyncProfile)>,
    pub scan: ScanResult,
    pub profile_idx: usize,
    pub should_quit: bool,
    pub flash: Option<String>,
    pub flash_ticks: u8,
    pub confirm_sync: bool,

    // Diff view
    pub diff_state: DiffState,
    pub diff_entry_idx: usize,
    pub diff_entry_filter: EntryFilter,

    // Progress view
    pub progress_rx: Option<Receiver<ProgressEvent>>,
    pub progress_state: Option<ProgressState>,

    // New profile wizard
    pub wizard: Option<NewProfileState>,
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
            confirm_sync: false,
            diff_state: DiffState::Idle,
            diff_entry_idx: 0,
            diff_entry_filter: EntryFilter::All,
            progress_rx: None,
            progress_state: None,
            wizard: None,
        })
    }

    pub fn selected_profile(&self) -> Option<&SyncProfile> {
        self.profiles.get(self.profile_idx).map(|(_, p)| p)
    }

    // ── Profiles ──────────────────────────────────────────────────────────

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

    // ── Diff ──────────────────────────────────────────────────────────────

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

    // ── Progress ──────────────────────────────────────────────────────────

    /// Drain pending progress events from the channel into `progress_state`.
    pub fn drain_progress(&mut self) {
        let rx = match self.progress_rx.as_ref() {
            Some(r) => r,
            None => return,
        };
        while let Ok(event) = rx.try_recv() {
            if let Some(ref mut ps) = self.progress_state {
                ps.handle_event(event);
            }
        }
    }

    // ── New profile wizard ────────────────────────────────────────────────

    pub fn enter_new_profile(&mut self) {
        let dap_ids = crate::dap::list().unwrap_or_else(|_| vec!["generic".to_owned()]);
        self.wizard = Some(NewProfileState::new(dap_ids));
        self.view = View::NewProfile;
    }

    // ── Shared ────────────────────────────────────────────────────────────

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
