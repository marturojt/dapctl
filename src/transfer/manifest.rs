use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    InProgress,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: Utf8PathBuf,
    pub size_bytes: u64,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
}

/// Append-only JSONL manifest for a single sync run.
///
/// Each state update is a new line; on resume the last written state for a
/// given path wins. Safe against crashes: partial writes are ignored.
pub struct Manifest {
    pub run_id: String,
    pub path: Utf8PathBuf,
    file: std::fs::File,
}

impl Manifest {
    /// Create a new manifest file and write all entries as `Pending`.
    pub fn create(
        run_id: &str,
        profile: &str,
        manifest_dir: &Utf8Path,
        entries: &[ManifestEntry],
    ) -> anyhow::Result<Self> {
        std::fs::create_dir_all(manifest_dir)
            .with_context(|| format!("cannot create manifest dir {manifest_dir}"))?;
        let path = manifest_dir.join(format!("{run_id}.jsonl"));
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("cannot open manifest {path}"))?;

        let started = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "?".to_owned());
        let header = serde_json::json!({
            "type": "header",
            "run_id": run_id,
            "profile": profile,
            "started_at": started,
        });
        writeln!(file, "{header}")?;

        for e in entries {
            writeln!(file, "{}", serde_json::to_string(e)?)?;
        }

        Ok(Self { run_id: run_id.to_owned(), path, file })
    }

    /// Append a state update for a single entry.
    pub fn update(&mut self, entry: &ManifestEntry) -> anyhow::Result<()> {
        writeln!(self.file, "{}", serde_json::to_string(entry)?)?;
        Ok(())
    }

    /// Read an existing manifest file and return the last known state per path.
    pub fn load_states(path: &Utf8Path) -> anyhow::Result<HashMap<Utf8PathBuf, State>> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("cannot open manifest {path}"))?;
        let mut states = HashMap::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.contains("\"type\"") {
                continue; // header line
            }
            if let Ok(entry) = serde_json::from_str::<ManifestEntry>(&line) {
                states.insert(entry.path, entry.state);
            }
        }
        Ok(states)
    }
}
