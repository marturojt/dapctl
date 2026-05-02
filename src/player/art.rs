use std::path::Path;

use image::DynamicImage;

/// Load cover art for a track. Tries embedded picture first, then folder art.
/// Returns `None` if nothing is found or decoding fails.
pub fn load_cover(track_path: &Path) -> Option<DynamicImage> {
    load_embedded(track_path).or_else(|| load_folder_art(track_path.parent()?))
}

fn load_embedded(path: &Path) -> Option<DynamicImage> {
    use lofty::prelude::TaggedFileExt;
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let pic = tag.pictures().first()?;
    image::load_from_memory(pic.data()).ok()
}

fn load_folder_art(dir: &Path) -> Option<DynamicImage> {
    const NAMES: &[&str] = &[
        "folder.jpg",
        "folder.jpeg",
        "cover.jpg",
        "cover.jpeg",
        "front.jpg",
        "front.jpeg",
        "album.jpg",
        "album.jpeg",
    ];
    // Case-insensitive match.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    let mut candidates: Vec<std::path::PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name_lc = p.file_name()?.to_string_lossy().to_lowercase();
            if NAMES.contains(&name_lc.as_str()) {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    candidates.sort();
    image::open(candidates.first()?).ok()
}
