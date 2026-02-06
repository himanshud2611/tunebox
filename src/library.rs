use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use walkdir::WalkDir;

use crate::metadata;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub track_number: Option<u32>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub format: String,
    pub file_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LibraryCache {
    directory: PathBuf,
    modified_time: u64,
    tracks: Vec<Track>,
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn format_from_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tunebox").join("library.json"))
}

fn load_cache(dir: &Path) -> Option<Vec<Track>> {
    let cache_file = cache_path()?;
    let data = std::fs::read_to_string(&cache_file).ok()?;
    let cache: LibraryCache = serde_json::from_str(&data).ok()?;

    if cache.directory != dir {
        return None;
    }

    // Check directory modification time
    let dir_modified = std::fs::metadata(dir)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if dir_modified > cache.modified_time {
        return None;
    }

    Some(cache.tracks)
}

fn save_cache(dir: &Path, tracks: &[Track]) {
    if let Some(cache_file) = cache_path() {
        if let Some(parent) = cache_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let dir_modified = std::fs::metadata(dir)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let cache = LibraryCache {
            directory: dir.to_path_buf(),
            modified_time: dir_modified,
            tracks: tracks.to_vec(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&cache) {
            let _ = std::fs::write(&cache_file, json);
        }
    }
}

pub fn scan_directory(dir: &Path) -> Result<Vec<Track>> {
    // Try loading from cache first
    if let Some(cached) = load_cache(dir) {
        return Ok(cached);
    }

    let mut tracks = Vec::new();

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_audio_file(path) {
            continue;
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let format = format_from_extension(path);

        match metadata::read_metadata(path) {
            Ok(meta) => {
                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                tracks.push(Track {
                    path: path.to_path_buf(),
                    title: meta.title.unwrap_or_else(|| filename),
                    artist: meta.artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                    album: meta.album.unwrap_or_else(|| "Unknown Album".to_string()),
                    duration: meta
                        .duration
                        .unwrap_or(Duration::ZERO)
                        .as_secs_f64(),
                    track_number: meta.track_number,
                    bitrate: meta.bitrate,
                    sample_rate: meta.sample_rate,
                    channels: meta.channels,
                    format,
                    file_size,
                });
            }
            Err(_) => {
                // Skip files we can't read metadata from but still add them
                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                tracks.push(Track {
                    path: path.to_path_buf(),
                    title: filename,
                    artist: "Unknown Artist".to_string(),
                    album: "Unknown Album".to_string(),
                    duration: 0.0,
                    track_number: None,
                    bitrate: None,
                    sample_rate: None,
                    channels: None,
                    format,
                    file_size,
                });
            }
        }
    }

    // Sort by artist -> album -> track number -> title
    tracks.sort_by(|a, b| {
        a.artist
            .to_lowercase()
            .cmp(&b.artist.to_lowercase())
            .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
            .then_with(|| a.track_number.cmp(&b.track_number))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });

    save_cache(dir, &tracks);
    Ok(tracks)
}

pub fn scan_single_file(path: &Path) -> Result<Vec<Track>> {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let format = format_from_extension(path);

    let meta = metadata::read_metadata(path).unwrap_or(crate::metadata::TrackMetadata {
        title: None,
        artist: None,
        album: None,
        track_number: None,
        duration: None,
        bitrate: None,
        sample_rate: None,
        channels: None,
        album_art: None,
    });

    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    Ok(vec![Track {
        path: path.to_path_buf(),
        title: meta.title.unwrap_or(filename),
        artist: meta.artist.unwrap_or_else(|| "Unknown Artist".to_string()),
        album: meta.album.unwrap_or_else(|| "Unknown Album".to_string()),
        duration: meta.duration.unwrap_or(Duration::ZERO).as_secs_f64(),
        track_number: meta.track_number,
        bitrate: meta.bitrate,
        sample_rate: meta.sample_rate,
        channels: meta.channels,
        format,
        file_size,
    }])
}
