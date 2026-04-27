use std::io::Read;

use anyhow::Context;
use camino::Utf8Path;

/// 2-second tolerance to handle FAT32 mtime precision.
const MTIME_TOLERANCE_NS: u128 = 2_000_000_000;

/// Read+hash buffer: 1 MiB keeps memory flat regardless of file size.
const HASH_BUF: usize = 1024 * 1024;

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

/// Verify by blake3 checksum. Streams in 1 MiB chunks — safe for any file size.
pub fn checksum(src: &Utf8Path, dst: &Utf8Path) -> anyhow::Result<bool> {
    Ok(hash_file(src)? == hash_file(dst)?)
}

/// Stream-hash a single file with blake3. Uses a fixed 1 MiB buffer.
pub fn hash_file(path: &Utf8Path) -> anyhow::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path.as_std_path())
        .with_context(|| format!("cannot open {path}"))?;
    let mut buf = vec![0u8; HASH_BUF];
    loop {
        let n = file.read(&mut buf).with_context(|| format!("read error on {path}"))?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}
