use camino::Utf8PathBuf;
use serde::Serialize;

pub mod heuristic;
pub mod removable;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A removable mount candidate.
#[derive(Debug, Clone, Serialize)]
pub struct Mount {
    pub mount_point: Utf8PathBuf,
    pub label: Option<String>,
    pub filesystem: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

/// A mount that has been matched against a DAP profile id.
#[derive(Debug, Clone, Serialize)]
pub struct IdentifiedDap {
    pub mount: Mount,
    pub dap_id: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Label matched a known DAP exactly.
    Exact,
    /// Label or marker file strongly suggests this DAP.
    Heuristic,
    /// No strong signal; fell back to a generic profile.
    Fallback,
}

// ---------------------------------------------------------------------------
// High-level scan
// ---------------------------------------------------------------------------

/// Enumerate removable drives and attempt to identify each one.
/// Unidentified mounts are included with `identified = None`.
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub identified: Vec<IdentifiedDap>,
    pub unidentified: Vec<Mount>,
}

pub fn run_scan() -> anyhow::Result<ScanResult> {
    let mounts = removable::enumerate()?;
    let mut identified = Vec::new();
    let mut unidentified = Vec::new();

    for mount in mounts {
        match heuristic::identify(&mount) {
            Some(d) => identified.push(d),
            None => unidentified.push(mount),
        }
    }

    tracing::info!(
        event = "scan_done",
        identified = identified.len(),
        unidentified = unidentified.len(),
    );

    Ok(ScanResult {
        identified,
        unidentified,
    })
}

// ---------------------------------------------------------------------------
// Destination resolution
// ---------------------------------------------------------------------------

/// Resolve a destination string from a sync profile.
///
/// `auto:<dap-id>` → scan for a connected drive matching that DAP and
///   append the DAP's configured `layout.music_root`.
/// Anything else → treat as a literal path.
pub fn resolve_destination(destination: &str) -> anyhow::Result<Utf8PathBuf> {
    let Some(dap_id) = destination.strip_prefix("auto:") else {
        return Ok(Utf8PathBuf::from(destination));
    };

    let mounts = removable::enumerate()?;
    for mount in &mounts {
        if let Some(id) = heuristic::identify(mount) {
            if id.dap_id == dap_id {
                let dap = crate::dap::load(dap_id)?;
                let music_root = dap.layout.music_root.trim_start_matches('/');
                let dest = if music_root.is_empty() {
                    mount.mount_point.clone()
                } else {
                    mount.mount_point.join(music_root)
                };
                tracing::debug!(
                    dap_id,
                    mount = %mount.mount_point,
                    dest = %dest,
                    "resolved auto destination"
                );
                return Ok(dest);
            }
        }
    }

    anyhow::bail!(
        "no connected drive identified as DAP {dap_id:?}; \
         run `dapctl scan` to see connected drives"
    )
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

pub fn fmt_bytes(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else {
        format!("{:.0} MB", bytes as f64 / MIB as f64)
    }
}
