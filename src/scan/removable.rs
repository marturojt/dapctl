use camino::Utf8PathBuf;
use sysinfo::Disks;

use super::Mount;

/// Enumerate removable drives using `sysinfo`.
///
/// Works on Linux, macOS, and Windows without conditional compilation.
/// Platform-specific improvements (lsblk, diskutil, SetupAPI) can be
/// layered in later for more reliable label/fs detection.
pub fn enumerate() -> anyhow::Result<Vec<Mount>> {
    let disks = Disks::new_with_refreshed_list();
    let mut mounts = Vec::new();

    for disk in &disks {
        if !disk.is_removable() {
            continue;
        }

        let mount_point = {
            let p = disk.mount_point();
            Utf8PathBuf::from_path_buf(p.to_path_buf())
                .unwrap_or_else(|p| Utf8PathBuf::from(p.to_string_lossy().into_owned()))
        };

        // sysinfo 0.30+ returns &OsStr for name and file_system
        let label = {
            let s = disk.name().to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        };

        let filesystem = {
            let s = disk.file_system().to_string_lossy().to_uppercase();
            if s.is_empty() { None } else { Some(s) }
        };

        mounts.push(Mount {
            mount_point,
            label,
            filesystem,
            total_bytes: Some(disk.total_space()),
            free_bytes: Some(disk.available_space()),
        });
    }

    Ok(mounts)
}
