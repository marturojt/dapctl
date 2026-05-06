//! iTunes Search API fallback for cover art.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

const ITUNES_SEARCH: &str = "https://itunes.apple.com/search";
pub const USER_AGENT: &str = concat!(
    "dapctl/",
    env!("CARGO_PKG_VERSION"),
    " (https://dapctl.com)"
);

/// Minimum sleep between iTunes requests (policy: 20 req/min → 3 s).
const ITUNES_RATE_SLEEP: Duration = Duration::from_millis(3_500);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    artwork_url100: Option<String>,
}

/// Search iTunes for `(artist, album)` and return a URL to the highest-
/// quality artwork available, or `None` if not found.
pub fn search_artwork_url(client: &Client, artist: &str, album: &str) -> Result<Option<String>> {
    let term = format!("{artist} {album}");

    let resp: SearchResponse = client
        .get(ITUNES_SEARCH)
        .header("User-Agent", USER_AGENT)
        .query(&[("term", term.as_str()), ("entity", "album"), ("limit", "5")])
        .send()
        .context("iTunes request failed")?
        .error_for_status()
        .context("iTunes error response")?
        .json()
        .context("iTunes JSON parse error")?;

    std::thread::sleep(ITUNES_RATE_SLEEP);

    let url = resp
        .results
        .into_iter()
        .find_map(|r| r.artwork_url100)
        // Upgrade from 100×100 thumbnail to 1000×1000
        .map(|u| u.replace("100x100bb", "1000x1000bb"));

    Ok(url)
}

/// Download image bytes from `url`.
pub fn fetch_url(client: &Client, url: &str) -> Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .context("image download failed")?
        .error_for_status()
        .context("image download error response")?
        .bytes()
        .context("reading image response body")?;

    Ok(bytes.to_vec())
}
