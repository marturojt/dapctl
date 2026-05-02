use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::player::queue::TrackInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// RFC3339 timestamp.
    pub ts: String,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Playback position in seconds when the entry was written.
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
}

impl HistoryEntry {
    pub fn from_track(track: &TrackInfo, position_secs: f64) -> Self {
        Self {
            ts: now_rfc3339(),
            path: track.path.to_string(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            position_secs,
            duration_secs: track.duration_secs,
        }
    }

    /// True when the track was abandoned mid-play and is worth resuming.
    pub fn is_resume_candidate(&self) -> bool {
        self.position_secs > 5.0
            && self
                .duration_secs
                .map_or(true, |d| self.position_secs < d - 10.0)
    }
}

fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn history_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")?;
    Some(dirs.data_local_dir().join("player").join("history.jsonl"))
}

/// Append one entry to the history file (non-fatal on error).
pub fn append(entry: &HistoryEntry) {
    let Some(path) = history_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    if let Ok(line) = serde_json::to_string(entry) {
        let _ = writeln!(file, "{line}");
    }
}

/// Load the last `n` history entries (most recent last).
pub fn load_last_n(n: usize) -> Vec<HistoryEntry> {
    let Some(path) = history_path() else {
        return Vec::new();
    };
    let Ok(file) = std::fs::File::open(&path) else {
        return Vec::new();
    };
    let mut entries: Vec<HistoryEntry> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect();
    if entries.len() > n {
        entries.drain(..entries.len() - n);
    }
    entries
}

/// Return the last entry if it is a good resume candidate.
pub fn load_resume() -> Option<HistoryEntry> {
    load_last_n(1)
        .into_iter()
        .next()
        .filter(|e| e.is_resume_candidate())
}
