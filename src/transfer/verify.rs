//! Post-copy verification: size+mtime or blake3 checksum.

use camino::Utf8Path;

pub fn size_mtime(_src: &Utf8Path, _dst: &Utf8Path) -> anyhow::Result<bool> {
    Ok(false)
}

pub fn checksum(_src: &Utf8Path, _dst: &Utf8Path) -> anyhow::Result<bool> {
    Ok(false)
}
