use camino::Utf8PathBuf;

/// A single entry in the playback queue.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    /// Absolute path on the local filesystem.
    pub path: Utf8PathBuf,
    /// Display name (filename without extension, or full path as fallback).
    pub title: String,
    /// Artist from tags, if available.
    pub artist: Option<String>,
    /// Album from tags, if available.
    pub album: Option<String>,
    /// Duration in seconds, if known.
    pub duration_secs: Option<f64>,
}

impl TrackInfo {
    pub fn from_path(path: Utf8PathBuf) -> Self {
        let title = path
            .file_stem()
            .unwrap_or(path.file_name().unwrap_or("unknown"))
            .to_owned();
        Self {
            path,
            title,
            artist: None,
            album: None,
            duration_secs: None,
        }
    }

    /// Populate artist/album/duration from audio tags (best-effort; never fails).
    pub fn with_tags(mut self) -> Self {
        use lofty::prelude::*;
        let Ok(tagged) = lofty::read_from_path(self.path.as_std_path()) else { return self };

        if let Some(tag) = tagged.primary_tag() {
            if let Some(a) = tag.artist() {
                self.artist = Some(a.into_owned());
            }
            if let Some(al) = tag.album() {
                self.album = Some(al.into_owned());
            }
            if let Some(title) = tag.title() {
                let t = title.into_owned();
                if !t.is_empty() {
                    self.title = t;
                }
            }
        }

        let props = tagged.properties();
        let secs = props.duration().as_secs_f64();
        if secs > 0.0 {
            self.duration_secs = Some(secs);
        }

        self
    }
}

/// Playback queue with repeat and shuffle support.
pub struct Queue {
    tracks: Vec<TrackInfo>,
    cursor: usize,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    shuffle_order: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            cursor: 0,
            repeat: RepeatMode::Off,
            shuffle: false,
            shuffle_order: Vec::new(),
        }
    }

    pub fn set(&mut self, tracks: Vec<TrackInfo>) {
        self.tracks = tracks;
        self.cursor = 0;
        self.rebuild_shuffle();
    }

    pub fn push(&mut self, track: TrackInfo) {
        self.tracks.push(track);
        self.rebuild_shuffle();
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.cursor = 0;
        self.shuffle_order.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn current(&self) -> Option<&TrackInfo> {
        let idx = self.effective_idx(self.cursor)?;
        self.tracks.get(idx)
    }

    pub fn tracks(&self) -> &[TrackInfo] {
        &self.tracks
    }

    /// Advance to next track. Returns `false` when the queue is exhausted
    /// (and repeat is Off), meaning playback should stop.
    pub fn advance(&mut self) -> bool {
        if self.tracks.is_empty() {
            return false;
        }
        match self.repeat {
            RepeatMode::One => true, // stay on same track
            RepeatMode::All => {
                self.cursor = (self.cursor + 1) % self.tracks.len();
                true
            }
            RepeatMode::Off => {
                if self.cursor + 1 < self.tracks.len() {
                    self.cursor += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn jump_to(&mut self, idx: usize) {
        if idx < self.tracks.len() {
            self.cursor = idx;
        }
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        self.rebuild_shuffle();
    }

    fn effective_idx(&self, cursor: usize) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.shuffle && !self.shuffle_order.is_empty() {
            self.shuffle_order.get(cursor).copied()
        } else {
            Some(cursor.min(self.tracks.len() - 1))
        }
    }

    fn rebuild_shuffle(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        if !self.shuffle || self.tracks.is_empty() {
            self.shuffle_order.clear();
            return;
        }
        // Deterministic shuffle seeded by track count + first path hash.
        let mut hasher = DefaultHasher::new();
        self.tracks.len().hash(&mut hasher);
        if let Some(t) = self.tracks.first() {
            t.path.hash(&mut hasher);
        }
        let seed = hasher.finish();

        let n = self.tracks.len();
        let mut order: Vec<usize> = (0..n).collect();
        let mut s = seed;
        for i in (1..n).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s as usize) % (i + 1);
            order.swap(i, j);
        }
        self.shuffle_order = order;
    }
}
