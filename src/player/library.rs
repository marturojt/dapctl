use std::collections::BTreeMap;

use camino::Utf8Path;

use crate::player::queue::TrackInfo;

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
        Self { artists: Vec::new() }
    }

    /// Build from a flat track list grouped by path structure.
    /// Root is stripped so grandparent = artist, parent = album.
    pub fn from_tracks(tracks: Vec<TrackInfo>, root: &Utf8Path) -> Self {
        let mut tree: BTreeMap<String, BTreeMap<String, Vec<TrackInfo>>> = BTreeMap::new();
        for track in tracks {
            let artist = derive_artist(&track.path, root);
            let album  = derive_album(&track.path, root);
            tree.entry(artist).or_default().entry(album).or_default().push(track);
        }
        let artists = tree
            .into_iter()
            .map(|(name, albums)| LibraryArtist {
                name,
                albums: albums
                    .into_iter()
                    .map(|(name, tracks)| LibraryAlbum { name, tracks })
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
            let album_match  = artist.albums.iter().any(|al| {
                al.name.to_lowercase().contains(&q)
                    || al.tracks.iter().any(|t| t.title.to_lowercase().contains(&q))
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
                        || album.tracks.iter().any(|t| t.title.to_lowercase().contains(&q));
                    if ok {
                        out.push(LibraryNode::Album { artist: ai, album: ali });
                    }
                }
            }
        }
        out
    }
}

// ── Path helpers ──────────────────────────────────────────────────────────────

fn rel_components<'a>(path: &'a Utf8Path, root: &Utf8Path) -> Vec<&'a str> {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_str())
        .collect()
}

fn derive_artist(path: &Utf8Path, root: &Utf8Path) -> String {
    let c = rel_components(path, root);
    if c.len() >= 2 {
        c[0].to_owned()
    } else {
        "Unknown Artist".to_owned()
    }
}

fn derive_album(path: &Utf8Path, root: &Utf8Path) -> String {
    let c = rel_components(path, root);
    if c.len() >= 3 {
        c[1].to_owned()
    } else {
        "Unknown Album".to_owned()
    }
}
