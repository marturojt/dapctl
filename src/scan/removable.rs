use camino::Utf8PathBuf;
use sysinfo::Disks;

use super::Mount;

/// Enumerate removable drives.
///
/// On Windows, `sysinfo::Disk::name()` returns the device name, not the
/// volume label. We call `GetVolumeInformationW` to get the actual label —
/// without it the heuristic cannot identify DAPs by their volume label.
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

        // On Windows, prefer GetVolumeInformationW for the label; fall back
        // to sysinfo's name which is the device path, not the volume label.
        let label = volume_label(&mount_point);

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

// ---------------------------------------------------------------------------
// Platform-specific volume label
// ---------------------------------------------------------------------------

/// Returns the volume label for the given mount point, or `None`.
fn volume_label(mount: &Utf8PathBuf) -> Option<String> {
    #[cfg(windows)]
    {
        windows_volume_label(mount)
    }
    #[cfg(not(windows))]
    {
        // On Linux/macOS sysinfo name is usually the device node (/dev/sdb1),
        // not the label. Returning None here; the heuristic falls back to
        // marker-file detection. Future improvement: parse /proc/mounts or
        // use IOKit to surface the label.
        let _ = mount;
        None
    }
}

#[cfg(windows)]
fn windows_volume_label(mount: &Utf8PathBuf) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    // Root path must end with `\`, e.g. "F:\\"
    let root_str = {
        let s = mount.as_str();
        if s.ends_with('\\') || s.ends_with('/') {
            s.to_owned()
        } else {
            format!("{s}\\")
        }
    };

    let root_wide: Vec<u16> = OsStr::new(&root_str)
        .encode_wide()
        .chain(Some(0))
        .collect();

    let mut label_buf = vec![0u16; 256];

    let ok = unsafe {
        GetVolumeInformationW(
            root_wide.as_ptr(),
            label_buf.as_mut_ptr(),
            label_buf.len() as u32,
            std::ptr::null_mut(), // serial number
            std::ptr::null_mut(), // max component len
            std::ptr::null_mut(), // fs flags
            std::ptr::null_mut(), // fs name
            0,
        )
    };

    if ok == 0 {
        return None;
    }

    let end = label_buf.iter().position(|&c| c == 0).unwrap_or(label_buf.len());
    let label = String::from_utf16(&label_buf[..end]).ok()?;
    if label.trim().is_empty() { None } else { Some(label.trim().to_owned()) }
}
