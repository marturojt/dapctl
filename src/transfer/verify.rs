use anyhow::Context;
use camino::Utf8Path;

/// 2-second tolerance to handle FAT32 mtime precision.
const MTIME_TOLERANCE_NS: u128 = 2_000_000_000;

/// Verify destination matches source by size and modification time.
/// Returns `Ok(true)` if they match within FAT32 mtime tolerance.
pub fn size_mtime(src: &Utf8Path, dst: &Utf8Path) -> anyhow::Result<bool> {
    let src_meta = std::fs::metadata(src.as_std_path())
        .with_context(|| format!("cannot stat {src}"))?;
    let dst_meta = std::fs::metadata(dst.as_std_path())
        .with_context(|| format!("cannot stat {dst}"))?;

    if src_meta.len() != dst_meta.len() {
        return Ok(false);
    }

    let mtime_ns = |meta: &std::fs::Metadata| -> u128 {
        meta.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    };

    let delta = mtime_ns(&src_meta).abs_diff(mtime_ns(&dst_meta));
    Ok(delta <= MTIME_TOLERANCE_NS)
}

/// Verify by blake3 checksum (slow on large files — only used when configured).
pub fn checksum(src: &Utf8Path, dst: &Utf8Path) -> anyhow::Result<bool> {
    let src_bytes = std::fs::read(src.as_std_path())
        .with_context(|| format!("cannot read {src}"))?;
    let dst_bytes = std::fs::read(dst.as_std_path())
        .with_context(|| format!("cannot read {dst}"))?;
    Ok(blake3::hash(&src_bytes) == blake3::hash(&dst_bytes))
}
