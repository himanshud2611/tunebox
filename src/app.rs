use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use rand::seq::SliceRandom;
use serde::Serialize;

use crate::albumart::AlbumArt;
use crate::audio::{AudioCommand, AudioEvent};
use crate::library::Track;
use crate::metadata;
use crate::visualizer::Visualizer;

/// Shared playback state for the remote control
#[derive(Clone, Serialize, Default)]
pub struct PlaybackState {
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
    pub track_album: Option<String>,
    pub progress: f64,
    pub duration: f64,
    pub is_playing: bool,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: String,
    pub theme: String,
    pub visualizer_mode: String,
    pub visualizer_bars: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "All",
            Self::One => "One",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Default,
    Dracula,
    Nord,
    Gruvbox,
    Neon,
}

impl Theme {
    pub fn cycle(self) -> Self {
        match self {
            Self::Default => Self::Dracula,
            Self::Dracula => Self::Nord,
            Self::Nord => Self::Gruvbox,
            Self::Gruvbox => Self::Neon,
            Self::Neon => Self::Default,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::Gruvbox => "Gruvbox",
            Self::Neon => "Neon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SleepTimer {
    pub end_time: Instant,
    pub fade_start: Instant,
    pub original_volume: f32,
    pub duration_mins: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackSpeed {
    Slow50,
    Slow75,
    Normal,
    Fast125,
    Fast150,
    Fast200,
}

impl PlaybackSpeed {
    pub fn cycle_up(self) -> Self {
        match self {
            Self::Slow50 => Self::Slow75,
            Self::Slow75 => Self::Normal,
            Self::Normal => Self::Fast125,
            Self::Fast125 => Self::Fast150,
            Self::Fast150 => Self::Fast200,
            Self::Fast200 => Self::Fast200,
        }
    }

    pub fn cycle_down(self) -> Self {
        match self {
            Self::Slow50 => Self::Slow50,
            Self::Slow75 => Self::Slow50,
            Self::Normal => Self::Slow75,
            Self::Fast125 => Self::Normal,
            Self::Fast150 => Self::Fast125,
            Self::Fast200 => Self::Fast150,
        }
    }

    pub fn as_f32(self) -> f32 {
        match self {
            Self::Slow50 => 0.5,
            Self::Slow75 => 0.75,
            Self::Normal => 1.0,
            Self::Fast125 => 1.25,
            Self::Fast150 => 1.5,
            Self::Fast200 => 2.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Slow50 => "0.5x",
            Self::Slow75 => "0.75x",
            Self::Normal => "1x",
            Self::Fast125 => "1.25x",
            Self::Fast150 => "1.5x",
            Self::Fast200 => "2x",
        }
    }
}

pub struct App {
    pub library: Vec<Track>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub playing_index: Option<usize>,
    pub is_playing: bool,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub progress: f64,
    pub duration: f64,
    pub visualizer: Visualizer,
    pub album_art: Option<AlbumArt>,
    pub search_mode: bool,
    pub search_query: String,
    pub show_info: bool,
    pub scroll_offset: usize,
    pub should_quit: bool,
    pub error_message: Option<String>,
    pub shuffle_order: Vec<usize>,

    // New features
    pub theme: Theme,
    pub sleep_timer: Option<SleepTimer>,
    pub speed: PlaybackSpeed,
    pub mini_mode: bool,

    // Channels
    pub cmd_tx: Sender<AudioCommand>,
    pub event_rx: Receiver<AudioEvent>,
    pub sample_rx: Receiver<Vec<f32>>,
}

impl App {
    pub fn new(
        library: Vec<Track>,
        cmd_tx: Sender<AudioCommand>,
        event_rx: Receiver<AudioEvent>,
        sample_rx: Receiver<Vec<f32>>,
    ) -> Self {
        let num_tracks = library.len();
        let filtered_indices: Vec<usize> = (0..num_tracks).collect();

        Self {
            library,
            filtered_indices,
            selected_index: 0,
            playing_index: None,
            is_playing: false,
            volume: 0.8,
            shuffle: false,
            repeat: RepeatMode::Off,
            progress: 0.0,
            duration: 0.0,
            visualizer: Visualizer::new(),
            album_art: None,
            search_mode: false,
            search_query: String::new(),
            show_info: false,
            scroll_offset: 0,
            should_quit: false,
            error_message: None,
            shuffle_order: Vec::new(),
            theme: Theme::default(),
            sleep_timer: None,
            speed: PlaybackSpeed::Normal,
            mini_mode: false,
            cmd_tx,
            event_rx,
            sample_rx,
        }
    }

    pub fn play_selected(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let lib_index = self.filtered_indices[self.selected_index];
        self.play_track(lib_index);
    }

    pub fn play_track(&mut self, index: usize) {
        if index >= self.library.len() {
            return;
        }
        let path = self.library[index].path.clone();
        self.playing_index = Some(index);
        self.is_playing = true;
        self.progress = 0.0;
        self.duration = self.library[index].duration;

        // Load album art
        self.load_album_art(&path);

        let _ = self.cmd_tx.send(AudioCommand::Play(path));
    }

    fn load_album_art(&mut self, path: &PathBuf) {
        if let Ok(meta) = metadata::read_metadata(path) {
            if let Some(art_data) = meta.album_art {
                self.album_art = AlbumArt::from_image_data(&art_data);
            } else {
                self.album_art = Some(AlbumArt::placeholder());
            }
        } else {
            self.album_art = Some(AlbumArt::placeholder());
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.playing_index.is_none() {
            // Nothing playing, play the selected track
            self.play_selected();
            return;
        }

        self.is_playing = !self.is_playing;
        if self.is_playing {
            let _ = self.cmd_tx.send(AudioCommand::Resume);
        } else {
            let _ = self.cmd_tx.send(AudioCommand::Pause);
        }
    }

    pub fn stop(&mut self) {
        let _ = self.cmd_tx.send(AudioCommand::Stop);
        self.is_playing = false;
        self.playing_index = None;
        self.progress = 0.0;
        self.duration = 0.0;
        self.album_art = Some(AlbumArt::placeholder());
    }

    pub fn next_track(&mut self) {
        if self.library.is_empty() {
            return;
        }

        let next_index = if self.shuffle {
            self.get_shuffle_next()
        } else if let Some(current) = self.playing_index {
            let next = current + 1;
            if next >= self.library.len() {
                match self.repeat {
                    RepeatMode::All => 0,
                    RepeatMode::Off => return,
                    RepeatMode::One => current,
                }
            } else {
                next
            }
        } else {
            0
        };

        self.play_track(next_index);
    }

    pub fn prev_track(&mut self) {
        if self.library.is_empty() {
            return;
        }

        // If we're more than 3 seconds in, restart the current track
        if self.progress > 3.0 {
            if let Some(idx) = self.playing_index {
                self.play_track(idx);
                return;
            }
        }

        let prev_index = if let Some(current) = self.playing_index {
            if current == 0 {
                match self.repeat {
                    RepeatMode::All => self.library.len() - 1,
                    _ => 0,
                }
            } else {
                current - 1
            }
        } else {
            0
        };

        self.play_track(prev_index);
    }

    pub fn seek_forward(&mut self) {
        let new_pos = (self.progress + 5.0).min(self.duration);
        let _ = self.cmd_tx.send(AudioCommand::Seek(new_pos));
    }

    pub fn seek_backward(&mut self) {
        let new_pos = (self.progress - 5.0).max(0.0);
        let _ = self.cmd_tx.send(AudioCommand::Seek(new_pos));
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
        let _ = self.cmd_tx.send(AudioCommand::SetVolume(self.volume));
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
        let _ = self.cmd_tx.send(AudioCommand::SetVolume(self.volume));
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        if self.shuffle {
            self.regenerate_shuffle();
        }
    }

    pub fn cycle_repeat(&mut self) {
        self.repeat = self.repeat.cycle();
    }

    pub fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_selection_down(&mut self) {
        if self.selected_index + 1 < self.filtered_indices.len() {
            self.selected_index += 1;
        }
    }

    pub fn toggle_search(&mut self) {
        self.search_mode = !self.search_mode;
        if !self.search_mode {
            self.search_query.clear();
            self.update_filter();
        }
    }

    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        self.update_filter();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.update_filter();
    }

    fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.library.len()).collect();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_indices = self
                .library
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    t.title.to_lowercase().contains(&query)
                        || t.artist.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len().saturating_sub(1);
        }
    }

    fn regenerate_shuffle(&mut self) {
        let mut rng = rand::thread_rng();
        self.shuffle_order = (0..self.library.len()).collect();
        self.shuffle_order.shuffle(&mut rng);
    }

    fn get_shuffle_next(&mut self) -> usize {
        if self.shuffle_order.is_empty() {
            self.regenerate_shuffle();
        }

        if let Some(current) = self.playing_index {
            if let Some(pos) = self.shuffle_order.iter().position(|&x| x == current) {
                let next_pos = pos + 1;
                if next_pos >= self.shuffle_order.len() {
                    match self.repeat {
                        RepeatMode::All => {
                            self.regenerate_shuffle();
                            self.shuffle_order[0]
                        }
                        RepeatMode::Off => current, // stay
                        RepeatMode::One => current,
                    }
                } else {
                    self.shuffle_order[next_pos]
                }
            } else {
                self.shuffle_order[0]
            }
        } else {
            self.shuffle_order[0]
        }
    }

    pub fn handle_track_finished(&mut self) {
        match self.repeat {
            RepeatMode::One => {
                if let Some(idx) = self.playing_index {
                    self.play_track(idx);
                }
            }
            _ => {
                self.next_track();
            }
        }
    }

    pub fn process_audio_events(&mut self) {
        // Process all pending audio events
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AudioEvent::Playing { duration } => {
                    if duration > 0.0 {
                        self.duration = duration;
                    }
                }
                AudioEvent::Progress(pos) => {
                    self.progress = pos;
                }
                AudioEvent::TrackFinished => {
                    self.handle_track_finished();
                }
                AudioEvent::Error(msg) => {
                    self.error_message = Some(msg);
                }
                AudioEvent::AudioData(_) => {
                    // Handled separately via sample_rx
                }
            }
        }

        // Process audio samples for visualizer
        let mut latest_samples = None;
        while let Ok(samples) = self.sample_rx.try_recv() {
            latest_samples = Some(samples);
        }
        if let Some(samples) = latest_samples {
            self.visualizer.process_samples(&samples);
        } else if self.is_playing {
            // Gentle decay when no new data
        } else {
            self.visualizer.decay();
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.playing_index.map(|i| &self.library[i])
    }

    // === New Feature Methods ===

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.cycle();
    }

    pub fn toggle_mini_mode(&mut self) {
        self.mini_mode = !self.mini_mode;
    }

    pub fn speed_up(&mut self) {
        self.speed = self.speed.cycle_up();
        let _ = self.cmd_tx.send(AudioCommand::SetSpeed(self.speed.as_f32()));
    }

    pub fn speed_down(&mut self) {
        self.speed = self.speed.cycle_down();
        let _ = self.cmd_tx.send(AudioCommand::SetSpeed(self.speed.as_f32()));
    }

    pub fn cycle_sleep_timer(&mut self) {
        // Cycle through: Off -> 15min -> 30min -> 45min -> 60min -> Off
        let new_duration = match &self.sleep_timer {
            None => Some(15),
            Some(t) => match t.duration_mins {
                15 => Some(30),
                30 => Some(45),
                45 => Some(60),
                _ => None,
            },
        };

        if let Some(mins) = new_duration {
            let now = Instant::now();
            let total_duration = Duration::from_secs(mins as u64 * 60);
            let fade_duration = Duration::from_secs(60); // Last minute is fade
            self.sleep_timer = Some(SleepTimer {
                end_time: now + total_duration,
                fade_start: now + total_duration - fade_duration,
                original_volume: self.volume,
                duration_mins: mins,
            });
        } else {
            // Restore volume if we had a timer
            if let Some(timer) = &self.sleep_timer {
                self.volume = timer.original_volume;
                let _ = self.cmd_tx.send(AudioCommand::SetVolume(self.volume));
            }
            self.sleep_timer = None;
        }
    }

    pub fn update_sleep_timer(&mut self) {
        if let Some(timer) = &self.sleep_timer {
            let now = Instant::now();

            if now >= timer.end_time {
                // Timer expired - stop playback
                let _ = self.cmd_tx.send(AudioCommand::Pause);
                self.is_playing = false;
                self.volume = timer.original_volume;
                let _ = self.cmd_tx.send(AudioCommand::SetVolume(self.volume));
                self.sleep_timer = None;
            } else if now >= timer.fade_start {
                // In fade period - gradually reduce volume
                let fade_total = timer.end_time.duration_since(timer.fade_start).as_secs_f32();
                let fade_remaining = timer.end_time.duration_since(now).as_secs_f32();
                let fade_ratio = fade_remaining / fade_total;
                let faded_volume = timer.original_volume * fade_ratio;
                self.volume = faded_volume;
                let _ = self.cmd_tx.send(AudioCommand::SetVolume(faded_volume));
            }
        }
    }

    pub fn sleep_timer_remaining(&self) -> Option<Duration> {
        self.sleep_timer.as_ref().map(|t| {
            let now = Instant::now();
            if now < t.end_time {
                t.end_time.duration_since(now)
            } else {
                Duration::ZERO
            }
        })
    }

    /// Get current playback state for the remote control
    pub fn playback_state(&self) -> PlaybackState {
        let (title, artist, album) = if let Some(track) = self.current_track() {
            (
                Some(track.title.clone()),
                Some(track.artist.clone()),
                Some(track.album.clone()),
            )
        } else {
            (None, None, None)
        };

        PlaybackState {
            track_title: title,
            track_artist: artist,
            track_album: album,
            progress: self.progress,
            duration: self.duration,
            is_playing: self.is_playing,
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat.label().to_string(),
            theme: self.theme.name().to_string(),
            visualizer_mode: self.visualizer.mode.label().to_string(),
            visualizer_bars: self.visualizer.bars.clone(),
        }
    }
}
