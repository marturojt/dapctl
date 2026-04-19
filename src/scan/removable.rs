//! Platform-specific enumeration of removable drives.
//!
//! v0.1 goal: Linux via `/proc/mounts` + `lsblk`, macOS via `IOKit`
//! (or `diskutil` shell-out as a stopgap), Windows via `GetLogicalDrives`
//! + `GetDriveTypeW`. For now, return an empty list on every platform.

use super::Mount;

pub fn enumerate() -> anyhow::Result<Vec<Mount>> {
    Ok(Vec::new())
}
