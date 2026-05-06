//! MusicBrainz search + Cover Art Archive fetch.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const CAA_BASE: &str = "https://coverartarchive.org";
pub const USER_AGENT: &str = concat!(
    "dapctl/",
    env!("CARGO_PKG_VERSION"),
    " (https://dapctl.com)"
);

/// Minimum sleep between MusicBrainz requests (policy: 1 req/s).
const MB_RATE_SLEEP: Duration = Duration::from_millis(1_100);
/// Minimum sleep after a CAA request.
const CAA_RATE_SLEEP: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
struct MbSearch {
    releases: Vec<MbRelease>,
}

#[derive(Deserialize)]
struct MbRelease {
    id: String,
}

/// Search MusicBrainz for the best-matching release MBID for `(artist, album)`.
/// Returns `None` when no results were found.
pub fn search_release(client: &Client, artist: &str, album: &str) -> Result<Option<String>> {
    let query = format!(
        "artist:\"{}\" AND release:\"{}\"",
        artist.replace('"', "\\\""),
        album.replace('"', "\\\""),
    );

    let resp: MbSearch = client
        .get(format!("{MB_BASE}/release/"))
        .header("User-Agent", USER_AGENT)
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "3")])
        .send()
        .context("MusicBrainz request failed")?
        .error_for_status()
        .context("MusicBrainz error response")?
        .json()
        .context("MusicBrainz JSON parse error")?;

    std::thread::sleep(MB_RATE_SLEEP);
    Ok(resp.releases.into_iter().next().map(|r| r.id))
}

/// Fetch the front cover image bytes for a release MBID from Cover Art Archive.
/// Returns `None` when the release has no front cover (404).
pub fn fetch_front(client: &Client, mbid: &str) -> Result<Option<Vec<u8>>> {
    let url = format!("{CAA_BASE}/release/{mbid}/front");

    let resp = client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .context("Cover Art Archive request failed")?;

    std::thread::sleep(CAA_RATE_SLEEP);

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let bytes = resp
        .error_for_status()
        .context("Cover Art Archive error response")?
        .bytes()
        .context("reading CAA response body")?;

    Ok(Some(bytes.to_vec()))
}
