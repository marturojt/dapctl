use std::collections::BTreeMap;

use camino::Utf8Path;

use crate::player::queue::TrackInfo;

/// Normalise an artist/album name into a grouping key that is:
/// - case-insensitive (all lowercase)
/// - diacritic-insensitive (à/á/â/ä → a, ñ → n, í → i, …)
///
/// The original display name is preserved separately; this key is only
/// used as the BTreeMap key so that "Kings Of Leon" and "Kings of Leon"
/// (or "Rosalía" and "Rosalia") collapse into one entry.
fn normalize_key(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ě' | 'ę' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
            'ñ' | 'ń' | 'ň' | 'ņ' => 'n',
            'ç' | 'ć' | 'č' => 'c',
            'ý' | 'ÿ' => 'y',
            'ž' | 'ź' | 'ż' => 'z',
            'š' | 'ś' | 'ş' => 's',
            'ð' | 'ď' | 'đ' => 'd',
            'ĺ' | 'ļ' | 'ľ' => 'l',
            'ŕ' | 'ř' | 'ŗ' => 'r',
            'ţ' | 'ť' => 't',
            'ğ' | 'ĝ' => 'g',
            c => c,
        })
        .collect()
}

// ── Data structures ───────────────────────────────────────────────────────────

pub struct LibraryIndex {
    pub artists: Vec<LibraryArtist>,
}

pub struct LibraryArtist {
    pub name: String,
    pub albums: Vec<LibraryAlbum>,
}

pub struct LibraryAlbum {
    pub name: String,
    pub tracks: Vec<TrackInfo>,
}

/// A row in the navigable flat list rendered by the library pane.
#[derive(Debug, Clone, Copy)]
pub enum LibraryNode {
    Artist(usize),
    Album { artist: usize, album: usize },
}

// ── LibraryIndex ──────────────────────────────────────────────────────────────

impl LibraryIndex {
    pub fn empty() -> Self {
        Self {
            artists: Vec::new(),
        }
    }

    /// Build from a flat track list.
    /// Groups by `album_artist` (falling back to `artist`, then path structure).
    /// Grouping keys are normalised (case + diacritics) so that e.g.
    /// "Kings Of Leon" / "Kings of Leon" and "Rosalía" / "Rosalia" merge
    /// into one entry. The display name is the first value seen for each key.
    pub fn from_tracks(tracks: Vec<TrackInfo>, root: &Utf8Path) -> Self {
        // BTreeMap<artist_key, (display_name, BTreeMap<album_key, (display_name, tracks)>)>
        type AlbumMap = BTreeMap<String, (String, Vec<TrackInfo>)>;
        let mut tree: BTreeMap<String, (String, AlbumMap)> = BTreeMap::new();

        for track in tracks {
            let artist_raw = group_artist(&track, root);
            let album_raw = group_album(&track, root);
            let artist_key = normalize_key(&artist_raw);
            let album_key = normalize_key(&album_raw);

            let (_, albums) = tree
                .entry(artist_key)
                .or_insert_with(|| (artist_raw, BTreeMap::new()));
            albums
                .entry(album_key)
                .or_insert_with(|| (album_raw, Vec::new()))
                .1
                .push(track);
        }

        let artists = tree
            .into_iter()
            .map(|(_, (name, albums))| LibraryArtist {
                name,
                albums: albums
                    .into_iter()
                    .map(|(_, (name, tracks))| LibraryAlbum { name, tracks })
                    .collect(),
            })
            .collect();
        Self { artists }
    }

    pub fn is_empty(&self) -> bool {
        self.artists.is_empty()
    }

    pub fn track_count(&self) -> usize {
        self.artists
            .iter()
            .flat_map(|a| a.albums.iter())
            .map(|al| al.tracks.len())
            .sum()
    }

    /// Build the navigable flat list given expanded state and search filter.
    pub fn build_flat(&self, expanded: &[bool], search: &str) -> Vec<LibraryNode> {
        let q = search.to_lowercase();
        let mut out = Vec::new();

        for (ai, artist) in self.artists.iter().enumerate() {
            let artist_match = q.is_empty() || artist.name.to_lowercase().contains(&q);
            let album_match = artist.albums.iter().any(|al| {
                al.name.to_lowercase().contains(&q)
                    || al
                        .tracks
                        .iter()
                        .any(|t| t.title.to_lowercase().contains(&q))
            });

            if !artist_match && !album_match {
                continue;
            }

            out.push(LibraryNode::Artist(ai));

            let show_albums = expanded.get(ai).copied().unwrap_or(false) || !q.is_empty();
            if show_albums {
                for (ali, album) in artist.albums.iter().enumerate() {
                    let ok = q.is_empty()
                        || artist_match
                        || album.name.to_lowercase().contains(&q)
                        || album
                            .tracks
                            .iter()
                            .any(|t| t.title.to_lowercase().contains(&q));
                    if ok {
                        out.push(LibraryNode::Album {
                            artist: ai,
                            album: ali,
                        });
                    }
                }
            }
        }
        out
    }
}

// ── Grouping helpers ──────────────────────────────────────────────────────────

fn group_artist(track: &TrackInfo, root: &Utf8Path) -> String {
    track
        .album_artist
        .as_deref()
        .or(track.artist.as_deref())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| path_artist(&track.path, root))
}

fn group_album(track: &TrackInfo, root: &Utf8Path) -> String {
    track
        .album
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| path_album(&track.path, root))
}

fn rel_components<'a>(path: &'a Utf8Path, root: &Utf8Path) -> Vec<&'a str> {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_str())
        .collect()
}

fn path_artist(path: &Utf8Path, root: &Utf8Path) -> String {
    let c = rel_components(path, root);
    if c.len() >= 2 {
        c[0].to_owned()
    } else {
        "Unknown Artist".to_owned()
    }
}

fn path_album(path: &Utf8Path, root: &Utf8Path) -> String {
    let c = rel_components(path, root);
    if c.len() >= 3 {
        c[1].to_owned()
    } else {
        "Unknown Album".to_owned()
    }
}
