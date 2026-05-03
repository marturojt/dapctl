use std::path::Path;

#[derive(Debug, Clone)]
pub struct LyricLine {
    /// Timestamp in milliseconds.
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    /// Lines sorted ascending by `time_ms`.
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    /// Parse an LRC file from its text content.
    pub fn from_lrc(content: &str) -> Self {
        let mut lines: Vec<LyricLine> = Vec::new();

        for raw in content.lines() {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            // Collect all leading [xx:xx.xx] timestamp tags.
            let mut timestamps: Vec<u64> = Vec::new();
            let mut rest = raw;
            while let Some(start) = rest.find('[') {
                let Some(end) = rest[start..].find(']') else {
                    break;
                };
                let tag = &rest[start + 1..start + end];
                rest = &rest[start + end + 1..];
                if let Some(ms) = parse_timestamp(tag) {
                    timestamps.push(ms);
                } else {
                    // Metadata tag or unrecognised — stop collecting timestamps.
                    break;
                }
            }
            let text = rest.trim().to_owned();
            for ms in timestamps {
                lines.push(LyricLine {
                    time_ms: ms,
                    text: text.clone(),
                });
            }
        }

        lines.sort_by_key(|l| l.time_ms);
        Self { lines }
    }

    /// Index of the active lyric line for `pos_secs` (the last line whose
    /// timestamp is ≤ pos_ms). Returns `None` when there are no lines or the
    /// position is before the first line.
    pub fn current_idx(&self, pos_secs: f64) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let pos_ms = (pos_secs * 1000.0) as u64;
        let idx = self.lines.partition_point(|l| l.time_ms <= pos_ms);
        if idx == 0 {
            None
        } else {
            Some(idx - 1)
        }
    }
}

/// Try to find a `.lrc` file alongside the given audio path.
pub fn find_lrc(track_path: &Path) -> Option<std::path::PathBuf> {
    let lrc = track_path.with_extension("lrc");
    if lrc.exists() {
        Some(lrc)
    } else {
        None
    }
}

/// Load and parse the `.lrc` file alongside `track_path`, if it exists.
pub fn load(track_path: &Path) -> Option<Lyrics> {
    let path = find_lrc(track_path)?;
    let content = std::fs::read_to_string(&path).ok()?;
    let lyrics = Lyrics::from_lrc(&content);
    if lyrics.lines.is_empty() {
        None
    } else {
        Some(lyrics)
    }
}

// ── Timestamp parser ──────────────────────────────────────────────────────────

/// Parse `mm:ss.xx`, `mm:ss.xxx`, or `mm:ss` → milliseconds.
fn parse_timestamp(s: &str) -> Option<u64> {
    let (min_str, rest) = s.split_once(':')?;
    let min: u64 = min_str.trim().parse().ok()?;

    let (sec_str, ms_str_opt) = match rest.split_once('.') {
        Some((s, m)) => (s, Some(m)),
        None => (rest, None),
    };
    let sec: u64 = sec_str.trim().parse().ok()?;

    let ms = match ms_str_opt {
        None => 0,
        Some(ms_str) => {
            let raw: u64 = ms_str.parse().ok()?;
            match ms_str.len() {
                1 => raw * 100,
                2 => raw * 10,
                3 => raw,
                _ => return None,
            }
        }
    };

    Some(min * 60_000 + sec * 1_000 + ms)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_lrc() {
        let src = "[00:12.34] Hello world\n[01:23.45] Second line\n";
        let lyr = Lyrics::from_lrc(src);
        assert_eq!(lyr.lines.len(), 2);
        assert_eq!(lyr.lines[0].time_ms, 12_340);
        assert_eq!(lyr.lines[0].text, "Hello world");
        assert_eq!(lyr.lines[1].time_ms, 83_450);
    }

    #[test]
    fn skip_metadata_tags() {
        let src = "[ar:Artist]\n[ti:Title]\n[00:05.00] First\n";
        let lyr = Lyrics::from_lrc(src);
        assert_eq!(lyr.lines.len(), 1);
        assert_eq!(lyr.lines[0].text, "First");
    }

    #[test]
    fn multiple_timestamps_same_line() {
        let src = "[00:10.00][01:10.00] Chorus\n";
        let lyr = Lyrics::from_lrc(src);
        assert_eq!(lyr.lines.len(), 2);
        assert!(lyr.lines.iter().all(|l| l.text == "Chorus"));
    }

    #[test]
    fn current_idx_before_first() {
        let src = "[00:10.00] First\n[00:20.00] Second\n";
        let lyr = Lyrics::from_lrc(src);
        assert_eq!(lyr.current_idx(5.0), None);
    }

    #[test]
    fn current_idx_advances() {
        let src = "[00:10.00] First\n[00:20.00] Second\n[00:30.00] Third\n";
        let lyr = Lyrics::from_lrc(src);
        assert_eq!(lyr.current_idx(10.0), Some(0));
        assert_eq!(lyr.current_idx(19.9), Some(0));
        assert_eq!(lyr.current_idx(20.0), Some(1));
        assert_eq!(lyr.current_idx(99.0), Some(2));
    }
}
