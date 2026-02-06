use anyhow::Result;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::picture::PictureType;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub duration: Option<Duration>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub album_art: Option<Vec<u8>>,
}

pub fn read_metadata(path: &Path) -> Result<TrackMetadata> {
    let tagged_file = Probe::open(path)?.read()?;

    let properties = tagged_file.properties();
    let duration = if properties.duration().as_secs() > 0 || properties.duration().subsec_millis() > 0 {
        Some(properties.duration())
    } else {
        None
    };
    let bitrate = properties.overall_bitrate();
    let sample_rate = properties.sample_rate();
    let channels = properties.channels();

    let mut title = None;
    let mut artist = None;
    let mut album = None;
    let mut track_number = None;
    let mut album_art = None;

    if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
        title = tag.title().map(|s| s.to_string());
        artist = tag.artist().map(|s| s.to_string());
        album = tag.album().map(|s| s.to_string());
        track_number = tag.track();

        // Extract album art
        if let Some(pic) = tag
            .pictures()
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| tag.pictures().first())
        {
            album_art = Some(pic.data().to_vec());
        }
    }

    Ok(TrackMetadata {
        title,
        artist,
        album,
        track_number,
        duration,
        bitrate,
        sample_rate,
        channels,
        album_art,
    })
}
