//! Top-level app state machine: which view is active, shared state, event loop.

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use camino::Utf8PathBuf;

use crate::config::{Mode, SyncProfile};
use crate::player::engine::{PlayerHandle, PlayerEvent};
use crate::scan::ScanResult;
use crate::transfer::{ProgressEvent, Stats};
use crate::tui::theme::Theme;
use crate::tui::views::player::PlayerState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Profiles,
    Diff,
    Progress,
    Log,
    NewProfile,
    Player,
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
    Mode,
    Confirm,
}

impl WizardStep {
    pub fn number(self) -> usize {
        match self {
            Self::Name => 1,
            Self::Source => 2,
            Self::Destination => 3,
            Self::Mode => 4,
            Self::Confirm => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "profile name",
            Self::Source => "source path",
            Self::Destination => "destination",
            Self::Mode => "sync mode",
            Self::Confirm => "confirm",
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Name => None,
            Self::Source => Some(Self::Name),
            Self::Destination => Some(Self::Source),
            Self::Mode => Some(Self::Destination),
            Self::Confirm => Some(Self::Mode),
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Name => Some(Self::Source),
            Self::Source => Some(Self::Destination),
            Self::Destination => Some(Self::Mode),
            Self::Mode => Some(Self::Confirm),
            Self::Confirm => None,
        }
    }
}

// ── File browser state ────────────────────────────────────────────────────────

pub struct FileBrowserState {
    /// Current directory. Empty when showing the drives list (Windows virtual root).
    pub current: camino::Utf8PathBuf,
    /// Subdirectory names (normal mode) or drive roots (drives-root mode).
    pub entries: Vec<String>,
    /// cursor == 0 → "[ ✓ select this directory ]" (only in normal mode).
    /// cursor == N → entries[N-1] in normal mode, entries[N] in drives-root mode.
    pub cursor: usize,
    /// True when showing available drives instead of a real directory (Windows).
    pub at_drives_root: bool,
}

impl FileBrowserState {
    /// Start at a concrete directory.
    pub fn new(start: camino::Utf8PathBuf) -> Self {
        let mut s = Self {
            current: start,
            entries: vec![],
            cursor: 0,
            at_drives_root: false,
        };
        s.refresh();
        s
    }

    /// Start at the virtual drives root (Windows) or "/" (Unix).
    pub fn drives_root() -> Self {
        let entries = available_drives();
        Self {
            current: camino::Utf8PathBuf::from(""),
            entries,
            cursor: 0,
            at_drives_root: true,
        }
    }

    pub fn refresh(&mut self) {
        if self.at_drives_root {
            self.entries = available_drives();
            return;
        }
        self.entries.clear();
        if let Ok(rd) = std::fs::read_dir(self.current.as_std_path()) {
            let mut dirs: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            dirs.sort_by_key(|a| a.to_lowercase());
            self.entries = dirs;
        }
    }

    /// Total selectable items: +1 for the "select" header in normal mode.
    pub fn total_items(&self) -> usize {
        if self.at_drives_root { self.entries.len() } else { self.entries.len() + 1 }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.total_items() {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Activate the highlighted item.
    /// Returns `true` only when the user chose "[ ✓ select this directory ]".
    pub fn enter_selected(&mut self) -> bool {
        if self.at_drives_root {
            if let Some(drive) = self.entries.get(self.cursor).cloned() {
                self.current = camino::Utf8PathBuf::from(&drive);
                self.at_drives_root = false;
                self.cursor = 0;
                self.refresh();
            }
            return false;
        }
        if self.cursor == 0 {
            return true; // user confirmed current dir
        }
        let idx = self.cursor - 1;
        if let Some(name) = self.entries.get(idx).cloned() {
            self.current = self.current.join(&name);
            self.cursor = 0;
            self.refresh();
        }
        false
    }

    /// Go up one directory level. From a drive root, returns to the drives list.
    pub fn go_up(&mut self) {
        if self.at_drives_root {
            return; // already at top
        }
        if is_fs_root(&self.current) {
            // Step up to virtual drives list.
            let prev = self.current.as_str().to_owned();
            self.at_drives_root = true;
            self.current = camino::Utf8PathBuf::from("");
            self.entries = available_drives();
            self.cursor = self.entries.iter().position(|e| e == &prev).unwrap_or(0);
            return;
        }
        let prev = self.current.file_name().unwrap_or("").to_owned();
        if let Some(parent) = self.current.parent() {
            self.current = camino::Utf8PathBuf::from(parent);
            self.refresh();
            self.cursor = self.entries.iter().position(|e| e == &prev)
                .map(|i| i + 1) // +1 for "select" item
                .unwrap_or(0);
        }
    }

    /// Human-readable label for the current location.
    pub fn location_label(&self) -> &str {
        if self.at_drives_root { "drives" } else { self.current.as_str() }
    }
}

fn is_fs_root(path: &camino::Utf8Path) -> bool {
    #[cfg(windows)]
    { let s = path.as_str(); s.len() <= 3 && s.contains(':') }
    #[cfg(not(windows))]
    { path.as_str() == "/" }
}

fn available_drives() -> Vec<String> {
    #[cfg(windows)]
    {
        ('A'..='Z')
            .map(|c| format!("{c}:\\"))
            .filter(|d| std::path::Path::new(d).exists())
            .collect()
    }
    #[cfg(not(windows))]
    { vec!["/".to_owned()] }
}

// ── Wizard state ──────────────────────────────────────────────────────────────

pub struct NewProfileState {
    pub step: WizardStep,
    pub name: tui_input::Input,
    pub source_browser: FileBrowserState,
    /// Index into [identified DAPs..., manual]. Last item is always "Browse…".
    pub dest_choice: usize,
    /// Active when the user chose "Browse…" in the destination list.
    pub dest_browser: Option<FileBrowserState>,
    pub mode_choice: usize,
    pub error: Option<String>,
    /// Set when the wizard was opened via "clone" — shows a hint in the Name step.
    pub cloned_from: Option<String>,
}

impl NewProfileState {
    pub fn new(scan: &crate::scan::ScanResult) -> Self {
        // Source: start at virtual drives root so the user can pick any drive.
        let source_browser = FileBrowserState::drives_root();

        // Destination browser: start at the first detected removable drive if
        // available; otherwise fall back to drives root.
        let dest_browser = scan.identified.first()
            .map(|id| FileBrowserState::new(camino::Utf8PathBuf::from(&id.mount.mount_point)))
            .or_else(|| scan.unidentified.first()
                .map(|m| FileBrowserState::new(camino::Utf8PathBuf::from(&m.mount_point))))
            .unwrap_or_else(FileBrowserState::drives_root);

        Self {
            step: WizardStep::Name,
            name: tui_input::Input::default(),
            source_browser,
            dest_choice: 0,
            dest_browser: Some(dest_browser),
            mode_choice: 0,
            error: None,
            cloned_from: None,
        }
    }

    /// Build a wizard state pre-populated from `profile`, used for cloning.
    pub fn from_clone(
        profile: &crate::config::SyncProfile,
        scan: &crate::scan::ScanResult,
    ) -> Self {
        let original_name = profile.profile.name.clone();

        // Source browser at the existing source directory.
        let src_path = camino::Utf8PathBuf::from(&profile.profile.source);
        let source_browser = if src_path.exists() {
            FileBrowserState::new(src_path)
        } else {
            FileBrowserState::drives_root()
        };

        // Destination: auto:dap_id → find index in identified; otherwise browse.
        let manual_idx = scan.identified.len();
        let dest = &profile.profile.destination;
        let (dest_choice, dest_browser) = if let Some(dap_id) = dest.strip_prefix("auto:") {
            let idx = scan.identified.iter().position(|id| id.dap_id == dap_id)
                .unwrap_or(manual_idx);
            let browser = scan.identified.first()
                .map(|id| FileBrowserState::new(camino::Utf8PathBuf::from(&id.mount.mount_point)))
                .unwrap_or_else(FileBrowserState::drives_root);
            (idx, Some(browser))
        } else {
            let dest_path = camino::Utf8PathBuf::from(dest.as_str());
            let browser = if dest_path.exists() {
                FileBrowserState::new(dest_path)
            } else {
                FileBrowserState::drives_root()
            };
            (manual_idx, Some(browser))
        };

        let mode_choice = match profile.profile.mode {
            Mode::Mirror => 1,
            _ => 0,
        };

        let suggested_name: tui_input::Input = format!("{original_name}-copy").into();

        Self {
            step: WizardStep::Name,
            name: suggested_name,
            source_browser,
            dest_choice,
            dest_browser,
            mode_choice,
            error: None,
            cloned_from: Some(original_name),
        }
    }

    pub fn source(&self) -> String {
        self.source_browser.current.to_string()
    }

    /// Resolved destination string for the current choice + scan.
    pub fn destination(&self, scan: &crate::scan::ScanResult) -> String {
        let manual_idx = scan.identified.len();
        if self.dest_choice == manual_idx {
            self.dest_browser.as_ref()
                .map(|b| b.current.to_string())
                .unwrap_or_default()
        } else {
            scan.identified.get(self.dest_choice)
                .map(|id| format!("auto:{}", id.dap_id))
                .unwrap_or_default()
        }
    }

    pub fn selected_dap(&self) -> &'static str {
        "generic"
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

    // Log view
    pub log_lines: Vec<LogEntry>,
    pub log_scroll: usize,
    pub log_run_id: String,

    // Player view
    pub player_state: Option<PlayerState>,
    pub player_handle: Option<PlayerHandle>,
    pub player_rx: Option<Receiver<PlayerEvent>>,
}

/// A single parsed JSONL log entry for display.
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub event: String,
    pub detail: String,
}

impl LogEntry {
    pub fn level_str(&self) -> &'static str {
        match self.level {
            LogLevel::Info  => "info",
            LogLevel::Warn  => "warn",
            LogLevel::Error => "error",
            LogLevel::Other => "?",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Other,
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
            log_lines: Vec::new(),
            log_scroll: 0,
            log_run_id: String::new(),
            player_state: None,
            player_handle: None,
            player_rx: None,
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
        self.wizard = Some(NewProfileState::new(&self.scan));
        self.view = View::NewProfile;
    }

    pub fn enter_clone_profile(&mut self) {
        let Some((_, profile)) = self.profiles.get(self.profile_idx) else { return };
        let profile = profile.clone();
        self.wizard = Some(NewProfileState::from_clone(&profile, &self.scan));
        self.view = View::NewProfile;
    }

    // ── Player view ───────────────────────────────────────────────────────

    pub fn enter_player(&mut self) {
        if self.player_state.is_none() {
            match crate::player::engine::spawn() {
                Some((handle, rx)) => {
                    self.player_state = Some(PlayerState::new(true));
                    self.player_handle = Some(handle);
                    self.player_rx = Some(rx);
                }
                None => {
                    self.player_state = Some(PlayerState::new(false));
                }
            }
        }
        self.view = View::Player;
    }

    pub fn drain_player(&mut self) {
        let rx = match self.player_rx.as_ref() {
            Some(r) => r,
            None => return,
        };
        if let Some(ref mut ps) = self.player_state {
            ps.drain_events(rx);
        }
    }

    // ── Log view ─────────────────────────────────────────────────────────

    pub fn load_log(&mut self) {
        let Some(path) = latest_jsonl_path() else {
            self.log_lines = vec![LogEntry {
                time: String::new(),
                level: LogLevel::Warn,
                event: "no runs found".into(),
                detail: String::new(),
            }];
            self.log_run_id = String::new();
            self.log_scroll = 0;
            return;
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.log_lines = vec![LogEntry {
                    time: String::new(),
                    level: LogLevel::Error,
                    event: format!("cannot read log: {e}"),
                    detail: String::new(),
                }];
                return;
            }
        };

        self.log_run_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();

        self.log_lines = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(parse_jsonl_line)
            .collect();

        // Start scrolled to the bottom.
        self.log_scroll = self.log_lines.len().saturating_sub(1);
        self.view = View::Log;
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

// ── Log helpers ───────────────────────────────────────────────────────────────

fn latest_jsonl_path() -> Option<std::path::PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")?;
    let runs_dir = dirs.data_local_dir().join("runs");
    let mut files: Vec<_> = std::fs::read_dir(&runs_dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .collect();
    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    files.last().map(|e| e.path())
}

fn parse_jsonl_line(line: &str) -> LogEntry {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return LogEntry {
            time: String::new(),
            level: LogLevel::Other,
            event: line.to_owned(),
            detail: String::new(),
        };
    };

    let time = v["ts"].as_str().unwrap_or("").to_owned();
    // Keep only HH:MM:SS from the RFC3339 timestamp.
    let time = time.get(11..19).unwrap_or(&time).to_owned();

    let level = match v["level"].as_str().unwrap_or("") {
        "info"  => LogLevel::Info,
        "warn"  => LogLevel::Warn,
        "error" => LogLevel::Error,
        _       => LogLevel::Other,
    };

    let fields = v["fields"].as_object();
    let event = fields
        .and_then(|f| f.get("event"))
        .and_then(|e| e.as_str())
        .unwrap_or("")
        .to_owned();

    // Build a compact detail string from the remaining fields.
    let detail = fields.map(|f| {
        f.iter()
            .filter(|(k, _)| k.as_str() != "event")
            .map(|(k, val)| {
                let v = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                format!("{k}={v}")
            })
            .collect::<Vec<_>>()
            .join("  ")
    }).unwrap_or_default();

    LogEntry { time, level, event, detail }
}
