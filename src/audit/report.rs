use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// High comes first in Ord so albums sort most-severe-first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MED "),
            Self::Low => write!(f, "LOW "),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    MissingTag {
        field: String,
        /// Number of tracks in the album that lack this field.
        affected: usize,
    },
    NoCover,
    FormatMix {
        formats: Vec<String>,
    },
    TrackGap {
        missing: Vec<u32>,
    },
}

impl Issue {
    pub fn severity(&self) -> Severity {
        match self {
            Self::NoCover => Severity::High,
            Self::MissingTag { field, .. } => match field.as_str() {
                "title" | "artist" | "album" => Severity::High,
                "track_num" => Severity::Medium,
                _ => Severity::Low,
            },
            Self::FormatMix { .. } => Severity::Medium,
            Self::TrackGap { .. } => Severity::Medium,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::NoCover => "no cover art".to_owned(),
            Self::MissingTag { field, affected } => {
                format!("{affected} track(s) missing {field} tag")
            }
            Self::FormatMix { formats } => format!("format mix: {}", formats.join(" + ")),
            Self::TrackGap { missing } => {
                let nums: Vec<_> = missing.iter().map(|n| n.to_string()).collect();
                if nums.len() <= 4 {
                    format!("track gaps: {}", nums.join(", "))
                } else {
                    format!(
                        "track gaps: {} (and {} more)",
                        nums[..3].join(", "),
                        nums.len() - 3
                    )
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumIssue {
    pub severity: Severity,
    #[serde(flatten)]
    pub issue: Issue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumReport {
    /// Absolute path to the album folder.
    pub path: PathBuf,
    /// Human-readable "Artist / Album" label derived from tags or folder name.
    pub display: String,
    pub track_count: usize,
    pub issues: Vec<AlbumIssue>,
}

impl AlbumReport {
    pub fn max_severity(&self) -> Option<Severity> {
        self.issues.iter().map(|i| i.severity).min()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub library: PathBuf,
    pub albums_scanned: usize,
    pub tracks_scanned: usize,
    pub albums_with_issues: usize,
    pub issues_total: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    /// Only albums that have at least one issue; sorted high → low severity.
    pub albums: Vec<AlbumReport>,
}
