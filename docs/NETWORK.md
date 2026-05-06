# Network policy

`dapctl` is **offline by default**. No network calls are made unless you
explicitly opt in with `--online`.

## Commands that use the network

| Command | Flag required | APIs called |
|---------|--------------|-------------|
| `dapctl cover fetch` | `--online` | MusicBrainz WS2, Cover Art Archive, iTunes Search |

## Endpoints

### MusicBrainz Web Service
- Base URL: `https://musicbrainz.org/ws/2/`
- Used for: release search (`artist + album → MBID`)
- Rate limit: 1 request/second (enforced by dapctl)
- User-Agent sent: `dapctl/<version> (https://dapctl.com)` — required by MB policy

### Cover Art Archive
- Base URL: `https://coverartarchive.org/`
- Used for: front cover image download by MBID
- Rate limit: none stated; dapctl sleeps 500 ms between requests
- No authentication required

### iTunes Search API
- URL: `https://itunes.apple.com/search`
- Used for: fallback when CAA has no image
- Rate limit: ~20 requests/minute (dapctl sleeps 3.5 s between requests)
- No authentication required

## Caching

All API responses are cached in `$XDG_CACHE_HOME/dapctl/metadata/cover_cache.json`
with a **30-day TTL**. A second `dapctl cover fetch --online` run on the same
library will use the cache and make no API calls for albums already resolved.

Negative results (albums not found in any source) are also cached so dapctl
does not re-query the API on every run.

## Privacy

- No telemetry, ever.
- Only the `artist` and `album` tag values of tracks without cover art are
  sent to external APIs.
- `--online` must be passed explicitly; there is no config option to make it
  the default.
