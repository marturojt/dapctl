//! Cover art fetcher: walks a library, finds album dirs without `folder.jpg`,
//! queries MusicBrainz → CAA then iTunes as fallback, saves `folder.jpg`.
//!
//! Network calls require `--online` (enforced by the CLI layer).
//! Results are cached in `$XDG_CACHE_HOME/dapctl/metadata/cover_cache.json`
//! with a 30-day TTL so repeated runs don't hammer the APIs.

pub mod itunes;
pub mod musicbrainz;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Constants ─────────────────────────────────────────────────────────────────

const AUDIO_EXTS: &[&str] = &[
    "flac", "mp3", "aac", "ogg", "opus", "wav", "alac", "m4a", "dsf", "dff", "wv", "wma", "aiff",
    "aif", "ape",
];

/// File names that count as "already has cover art".
const COVER_NAMES: &[&str] = &[
    "folder.jpg",
    "folder.jpeg",
    "cover.jpg",
    "cover.jpeg",
    "front.jpg",
    "front.jpeg",
    "album.jpg",
    "album.jpeg",
];

const CACHE_TTL_SECS: u64 = 30 * 24 * 3600; // 30 days

// ── Public types ──────────────────────────────────────────────────────────────

pub struct FetchOptions {
    pub path: PathBuf,
}

pub struct FetchStats {
    pub albums_scanned: usize,
    pub already_have: usize,
    pub fetched: usize,
    pub not_found: usize,
    pub errors: usize,
}

// ── Cache ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct CoverCache {
    /// Key: `"artist\x00album"` (NUL-separated, both lowercased).
    /// Value: resolved artwork URL (empty = confirmed not found), timestamp.
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CacheEntry {
    /// Empty string means "known not found".
    url: String,
    ts: u64,
}

impl CoverCache {
    fn load() -> Self {
        cache_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = cache_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn get(&self, key: &str) -> Option<&CacheEntry> {
        let entry = self.entries.get(key)?;
        let now = now_secs();
        if now.saturating_sub(entry.ts) < CACHE_TTL_SECS {
            Some(entry)
        } else {
            None
        }
    }

    fn set(&mut self, key: String, url: String) {
        self.entries.insert(
            key,
            CacheEntry {
                url,
                ts: now_secs(),
            },
        );
    }
}

fn cache_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "dapctl")
        .map(|d| d.cache_dir().join("metadata").join("cover_cache.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Walk `opts.path`, find album dirs without cover art, and fetch + save
/// `folder.jpg` for each.  `progress` is called with a human-readable status
/// line for each album processed.
pub fn fetch(opts: &FetchOptions, progress: impl Fn(&str)) -> Result<FetchStats> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(musicbrainz::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    let mut cache = CoverCache::load();
    let album_dirs = collect_album_dirs(&opts.path);

    let mut stats = FetchStats {
        albums_scanned: album_dirs.len(),
        already_have: 0,
        fetched: 0,
        not_found: 0,
        errors: 0,
    };

    for dir in &album_dirs {
        if album_has_cover(dir) {
            stats.already_have += 1;
            continue;
        }

        let (artist, album) = extract_artist_album(dir);
        if artist.is_empty() && album.is_empty() {
            progress(&format!("  skip  {} (no tags)", dir_label(dir)));
            stats.not_found += 1;
            continue;
        }

        let cache_key = format!("{}\x00{}", artist.to_lowercase(), album.to_lowercase());

        // Check cache first — cache stores the artwork URL (or empty = not found).
        if let Some(entry) = cache.get(&cache_key) {
            if entry.url.is_empty() {
                stats.not_found += 1;
                continue;
            }
            let url = entry.url.clone();
            match fetch_url_and_save(&client, &url, dir) {
                Ok(()) => {
                    progress(&format!("  \u{2713}  {} (cached)", dir_label(dir)));
                    stats.fetched += 1;
                }
                Err(e) => {
                    progress(&format!("  !  {} \u{2014} {e}", dir_label(dir)));
                    stats.errors += 1;
                }
            }
            continue;
        }

        // Try MusicBrainz → Cover Art Archive (returns actual image bytes).
        let mb_result = try_musicbrainz(&client, &artist, &album);

        match mb_result {
            Err(e) => {
                progress(&format!(
                    "  !  {} \u{2014} MusicBrainz error: {e}",
                    dir_label(dir)
                ));
                stats.errors += 1;
            }
            Ok(Some(bytes)) => {
                // Got bytes directly from CAA — store a synthetic URL in the cache
                // so subsequent runs can re-fetch without querying MB again.
                let caa_url = format!(
                    "https://coverartarchive.org/release/cached/{}",
                    cache_key.replace('\x00', "--")
                );
                cache.set(cache_key, caa_url);
                cache.save();
                match save_jpeg(bytes, dir) {
                    Ok(()) => {
                        progress(&format!("  \u{2713}  {} (MusicBrainz)", dir_label(dir)));
                        stats.fetched += 1;
                    }
                    Err(e) => {
                        progress(&format!("  !  {} \u{2014} save error: {e}", dir_label(dir)));
                        stats.errors += 1;
                    }
                }
            }
            Ok(None) => {
                // Fallback: iTunes Search API — returns a URL.
                match itunes::search_artwork_url(&client, &artist, &album) {
                    Err(e) => {
                        progress(&format!(
                            "  !  {} \u{2014} iTunes error: {e}",
                            dir_label(dir)
                        ));
                        stats.errors += 1;
                        cache.set(cache_key, String::new());
                        cache.save();
                    }
                    Ok(None) => {
                        progress(&format!(
                            "  \u{2717}  {} \u{2014} not found",
                            dir_label(dir)
                        ));
                        cache.set(cache_key, String::new());
                        cache.save();
                        stats.not_found += 1;
                    }
                    Ok(Some(url)) => {
                        cache.set(cache_key, url.clone());
                        cache.save();
                        match fetch_url_and_save(&client, &url, dir) {
                            Ok(()) => {
                                progress(&format!("  \u{2713}  {} (iTunes)", dir_label(dir)));
                                stats.fetched += 1;
                            }
                            Err(e) => {
                                progress(&format!(
                                    "  !  {} \u{2014} save error: {e}",
                                    dir_label(dir)
                                ));
                                stats.errors += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(stats)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn album_has_cover(dir: &Path) -> bool {
    COVER_NAMES.iter().any(|n| dir.join(n).exists())
}

/// Walk `root` and collect the set of unique parent directories that contain
/// at least one audio file (each directory = one album).
fn collect_album_dirs(root: &Path) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut dirs = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !AUDIO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        if let Some(parent) = entry.path().parent() {
            if seen.insert(parent.to_path_buf()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }

    dirs.sort();
    dirs
}

/// Read audio tags from all files in `dir` and return the most-common
/// `(artist, album)` pair.
fn extract_artist_album(dir: &Path) -> (String, String) {
    use lofty::prelude::{Accessor, TaggedFileExt};

    let mut artists: HashMap<String, usize> = HashMap::new();
    let mut albums: HashMap<String, usize> = HashMap::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (String::new(), String::new()),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !AUDIO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        if let Ok(tagged) = lofty::read_from_path(&path) {
            let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
            if let Some(tag) = tag {
                // Try AlbumArtist first, fall back to Artist.
                let artist_val = tag
                    .get_string(&lofty::tag::ItemKey::AlbumArtist)
                    .map(str::to_owned)
                    .or_else(|| tag.artist().map(|a| a.into_owned()));
                if let Some(a) = artist_val {
                    if !a.is_empty() {
                        *artists.entry(a).or_insert(0) += 1;
                    }
                }
                if let Some(al) = tag.album() {
                    let al = al.into_owned();
                    if !al.is_empty() {
                        *albums.entry(al).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let artist = artists
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(a, _)| a)
        .unwrap_or_default();
    let album = albums
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(a, _)| a)
        .unwrap_or_default();

    (artist, album)
}

/// Try MusicBrainz search then Cover Art Archive.
/// Returns the image bytes on success, `Ok(None)` when no cover was found.
fn try_musicbrainz(
    client: &reqwest::blocking::Client,
    artist: &str,
    album: &str,
) -> Result<Option<Vec<u8>>> {
    let mbid = match musicbrainz::search_release(client, artist, album)? {
        Some(id) => id,
        None => return Ok(None),
    };

    musicbrainz::fetch_front(client, &mbid)
}

/// Download `url`, convert to JPEG if needed, and write to `<dir>/folder.jpg`.
fn fetch_url_and_save(client: &reqwest::blocking::Client, url: &str, dir: &Path) -> Result<()> {
    let bytes = itunes::fetch_url(client, url)?;
    save_jpeg(bytes, dir)
}

/// Convert bytes to JPEG (pass-through if already JPEG) and write `folder.jpg`.
fn save_jpeg(bytes: Vec<u8>, dir: &Path) -> Result<()> {
    let jpeg = to_jpeg(&bytes).context("image conversion failed")?;
    let dest = dir.join("folder.jpg");
    std::fs::write(&dest, &jpeg).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// Convert image bytes to JPEG.  JPEG input is returned as-is to avoid
/// generation loss; other formats (PNG, WebP, …) are re-encoded.
fn to_jpeg(bytes: &[u8]) -> Result<Vec<u8>> {
    // JPEG magic: FF D8
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return Ok(bytes.to_vec());
    }
    let img = image::load_from_memory(bytes).context("unsupported image format")?;
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Jpeg)
        .context("JPEG encoding failed")?;
    Ok(out.into_inner())
}

fn dir_label(dir: &Path) -> String {
    // Show last two path components for readability.
    let components: Vec<_> = dir.components().collect();
    let n = components.len();
    if n >= 2 {
        format!(
            "{}/{}",
            components[n - 2].as_os_str().to_string_lossy(),
            components[n - 1].as_os_str().to_string_lossy()
        )
    } else {
        dir.display().to_string()
    }
}
