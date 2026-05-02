use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::time::UNIX_EPOCH;

use camino::Utf8PathBuf;
use rusqlite::{Connection, params};

use crate::player::library::LibraryIndex;
use crate::player::queue::TrackInfo;

const SCHEMA_VERSION: u32 = 1;

const AUDIO_EXTS: &[&str] = &[
    "flac", "mp3", "aac", "ogg", "opus", "wav", "alac", "m4a",
    "dsf", "dff", "wv", "wma", "aiff", "aif", "ape",
];

pub enum ScanEvent {
    Progress { done: usize, total: usize },
    Done(LibraryIndex),
    Error(String),
}

pub fn spawn_scan(root: Utf8PathBuf, tx: Sender<ScanEvent>) {
    std::thread::spawn(move || {
        if let Err(e) = run_scan(root, &tx) {
            let _ = tx.send(ScanEvent::Error(e.to_string()));
        }
    });
}

// ── DB helpers ────────────────────────────────────────────────────────────────

fn db_path(root: &Utf8PathBuf) -> std::path::PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    root.as_str().hash(&mut h);
    let hash = h.finish();

    directories::ProjectDirs::from("", "", "dapctl")
        .expect("cannot determine app data directory")
        .data_local_dir()
        .join(format!("lib_{hash:016x}.db"))
}

fn open_db(path: &std::path::Path) -> anyhow::Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;

    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        conn.execute_batch("DROP TABLE IF EXISTS tracks;")?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tracks (
            path          TEXT    PRIMARY KEY,
            mtime_ns      INTEGER NOT NULL,
            size          INTEGER NOT NULL,
            title         TEXT,
            artist        TEXT,
            album_artist  TEXT,
            album         TEXT,
            track_number  INTEGER,
            disc_number   INTEGER,
            year          INTEGER,
            genre         TEXT,
            duration_secs REAL,
            sample_rate_hz INTEGER,
            bit_depth     INTEGER,
            bitrate_kbps  INTEGER,
            channels      INTEGER
        );",
    )?;

    Ok(conn)
}

// ── Main scan logic ───────────────────────────────────────────────────────────

fn run_scan(root: Utf8PathBuf, tx: &Sender<ScanEvent>) -> anyhow::Result<()> {
    let mut conn = open_db(&db_path(&root))?;

    // 1. Walk audio files
    let paths: Vec<Utf8PathBuf> = walkdir::WalkDir::new(root.as_std_path())
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|ext| AUDIO_EXTS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .filter_map(|e| Utf8PathBuf::from_path_buf(e.into_path()).ok())
        .collect();

    let total = paths.len();
    let _ = tx.send(ScanEvent::Progress { done: 0, total });

    // 2. Find stale/missing entries (needs tag re-read).
    //    Collect into an owned Vec so the statement (and its conn borrow) drops
    //    before we need &mut conn for the transaction.
    let stale: Vec<(Utf8PathBuf, i64, i64)> = {
        let mut stmt = conn.prepare("SELECT mtime_ns, size FROM tracks WHERE path = ?1")?;
        paths
            .iter()
            .filter_map(|p| {
                let meta = std::fs::metadata(p.as_std_path()).ok()?;
                let size = meta.len() as i64;
                let mtime_ns = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as i64)
                    .unwrap_or(0);

                let cached = stmt
                    .query_row(params![p.as_str()], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                    })
                    .ok();

                match cached {
                    Some((cm, cs)) if cm == mtime_ns && cs == size => None,
                    _ => Some((p.clone(), mtime_ns, size)),
                }
            })
            .collect()
    }; // stmt dropped here

    // 3. Read tags in parallel for stale files
    use rayon::prelude::*;
    let updates: Vec<(TrackInfo, i64, i64)> = stale
        .into_par_iter()
        .map(|(path, mtime_ns, size)| {
            let track = TrackInfo::from_path(path).with_tags();
            (track, mtime_ns, size)
        })
        .collect();

    // 4. Upsert in a single transaction
    if !updates.is_empty() {
        let tx_db = conn.transaction()?;
        for (track, mtime_ns, size) in &updates {
            tx_db.execute(
                "INSERT OR REPLACE INTO tracks \
                 (path, mtime_ns, size, title, artist, album_artist, album, \
                  track_number, disc_number, year, genre, \
                  duration_secs, sample_rate_hz, bit_depth, bitrate_kbps, channels) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
                params![
                    track.path.as_str(),
                    mtime_ns,
                    size,
                    track.title.as_str(),
                    track.artist.as_deref(),
                    track.album_artist.as_deref(),
                    track.album.as_deref(),
                    track.track_number.map(|n| n as i64),
                    track.disc_number.map(|n| n as i64),
                    track.year.map(|n| n as i64),
                    track.genre.as_deref(),
                    track.duration_secs,
                    track.sample_rate_hz.map(|n| n as i64),
                    track.bit_depth.map(|n| n as i64),
                    track.bitrate_kbps.map(|n| n as i64),
                    track.channels.map(|n| n as i64),
                ],
            )?;
        }
        tx_db.commit()?;
    }

    // 5. Prune deleted paths from DB
    {
        let path_set: HashSet<&str> = paths.iter().map(|p| p.as_str()).collect();

        let to_delete: Vec<String> = {
            let mut stmt = conn.prepare("SELECT path FROM tracks")?;
            let all: Vec<String> = stmt.query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            all.into_iter().filter(|p| !path_set.contains(p.as_str())).collect()
        };

        if !to_delete.is_empty() {
            let tx_db = conn.transaction()?;
            for p in &to_delete {
                tx_db.execute("DELETE FROM tracks WHERE path = ?1", params![p])?;
            }
            tx_db.commit()?;
        }
    }

    // 6. Load all tracks ordered for the library tree
    let tracks: Vec<TrackInfo> = {
        let mut stmt = conn.prepare(
            "SELECT path, title, artist, album_artist, album,
                    track_number, disc_number, year, genre,
                    duration_secs, sample_rate_hz, bit_depth, bitrate_kbps, channels
             FROM tracks
             ORDER BY LOWER(COALESCE(album_artist, artist, '')),
                      LOWER(COALESCE(album, '')),
                      COALESCE(disc_number, 1),
                      COALESCE(track_number, 999),
                      LOWER(title)",
        )?;

        struct Row {
            path: String,
            title: Option<String>,
            artist: Option<String>,
            album_artist: Option<String>,
            album: Option<String>,
            track_number: Option<i64>,
            disc_number: Option<i64>,
            year: Option<i64>,
            genre: Option<String>,
            duration_secs: Option<f64>,
            sample_rate_hz: Option<i64>,
            bit_depth: Option<i64>,
            bitrate_kbps: Option<i64>,
            channels: Option<i64>,
        }

        let rows: Vec<Row> = stmt.query_map([], |r| {
            Ok(Row {
                path:           r.get(0)?,
                title:          r.get(1)?,
                artist:         r.get(2)?,
                album_artist:   r.get(3)?,
                album:          r.get(4)?,
                track_number:   r.get(5)?,
                disc_number:    r.get(6)?,
                year:           r.get(7)?,
                genre:          r.get(8)?,
                duration_secs:  r.get(9)?,
                sample_rate_hz: r.get(10)?,
                bit_depth:      r.get(11)?,
                bitrate_kbps:   r.get(12)?,
                channels:       r.get(13)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        rows.into_iter().map(|row| {
            let path = Utf8PathBuf::from(&row.path);
            let title = row.title.unwrap_or_else(|| {
                path.file_stem().unwrap_or("unknown").to_owned()
            });
            TrackInfo {
                path,
                title,
                artist:         row.artist,
                album_artist:   row.album_artist,
                album:          row.album,
                track_number:   row.track_number.map(|n| n as u32),
                disc_number:    row.disc_number.map(|n| n as u32),
                year:           row.year.map(|n| n as u32),
                genre:          row.genre,
                duration_secs:  row.duration_secs,
                sample_rate_hz: row.sample_rate_hz.map(|n| n as u32),
                bit_depth:      row.bit_depth.map(|n| n as u8),
                bitrate_kbps:   row.bitrate_kbps.map(|n| n as u32),
                channels:       row.channels.map(|n| n as u8),
            }
        })
        .collect()
    };

    let index = LibraryIndex::from_tracks(tracks, &root);
    let _ = tx.send(ScanEvent::Done(index));
    Ok(())
}
