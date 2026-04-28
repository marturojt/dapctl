use std::io;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

use crate::transfer::verify::hash_file;

/// Compute a stable cache key for `(source_file, ffmpeg_params)`.
///
/// Key = hex(blake3(source_content || params_bytes)).  Changing either the
/// source file or the params invalidates the cache entry.
pub fn cache_key(src: &Utf8Path, params: &str) -> anyhow::Result<String> {
    let src_hash = hash_file(src)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(src_hash.as_bytes());
    hasher.update(params.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

/// File-system cache for transcoded outputs.
///
/// Layout: `<dir>/<key[..2]>/<key>.<ext>`  (256 prefix shards, like git objects).
pub struct Cache {
    dir: Utf8PathBuf,
}

impl Cache {
    pub fn new(dir: Utf8PathBuf) -> Self {
        Self { dir }
    }

    /// Returns the cached file path if it exists and is non-empty.
    pub fn get(&self, key: &str, ext: &str) -> Option<Utf8PathBuf> {
        let path = self.entry_path(key, ext);
        match path.metadata() {
            Ok(m) if m.len() > 0 => Some(path),
            _ => None,
        }
    }

    /// Copy `src` into the cache as the entry for `(key, ext)`.
    /// Uses `std::fs::copy` so cross-device writes work.
    pub fn store(&self, key: &str, ext: &str, src: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
        let dest = self.entry_path(key, ext);
        std::fs::create_dir_all(dest.parent().expect("cache entry has parent"))
            .with_context(|| format!("cannot create cache shard dir for {dest}"))?;

        // Write via a temp file so a partial write is never visible as a hit.
        let tmp = dest.with_extension(format!("{}.tmp", ext));
        std::fs::copy(src.as_std_path(), tmp.as_std_path())
            .with_context(|| format!("cannot write cache tmp {tmp}"))?;
        std::fs::rename(tmp.as_std_path(), dest.as_std_path())
            .with_context(|| format!("cannot commit cache entry {dest}"))?;

        Ok(dest)
    }

    fn entry_path(&self, key: &str, ext: &str) -> Utf8PathBuf {
        let prefix = &key[..2.min(key.len())];
        self.dir
            .join(prefix)
            .join(format!("{}.{}", key, ext.to_lowercase()))
    }
}

/// Copy a file from `src` to `dst`, creating parent directories as needed.
/// Returns the number of bytes copied.
pub fn copy_to(src: &Utf8Path, dst: &Utf8Path) -> io::Result<u64> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src.as_std_path(), dst.as_std_path())
}
