//! Top-level app state machine: which view is active, shared state, event loop.

use crate::config::SyncProfile;
use crate::scan::ScanResult;
use crate::tui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Profiles,
    Diff,
    Progress,
    Log,
}

pub struct App {
    pub view: View,
    pub theme: Theme,
    /// Loaded sync profiles: (display-name, profile).
    pub profiles: Vec<(String, SyncProfile)>,
    pub scan: ScanResult,
    /// Selected index into `profiles`.
    pub profile_idx: usize,
    pub should_quit: bool,
    /// Transient status message shown at the bottom (e.g., key hints override).
    pub flash: Option<String>,
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

        // Non-fatal: scan may fail if sysinfo unavailable.
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
        })
    }

    pub fn selected_profile(&self) -> Option<&SyncProfile> {
        self.profiles.get(self.profile_idx).map(|(_, p)| p)
    }

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

    /// Refresh the scan result (e.g. when user presses `r`).
    pub fn refresh_scan(&mut self) {
        if let Ok(s) = crate::scan::run_scan() {
            self.scan = s;
        }
    }
}
