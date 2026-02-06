use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

/// Commands sent from TUI to audio thread
#[derive(Debug)]
pub enum AudioCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(f64),
    SetVolume(f32),
    SetSpeed(f32),
}

/// Events sent from audio thread to TUI
#[derive(Debug)]
pub enum AudioEvent {
    Playing {
        duration: f64,
    },
    Progress(f64),
    AudioData(Vec<f32>),
    TrackFinished,
    Error(String),
}

/// Wraps a Source to capture samples for the visualizer and track progress
struct CaptureSource<S> {
    inner: S,
    sample_tx: Sender<Vec<f32>>,
    progress_counter: Arc<AtomicU64>,
    is_finished: Arc<AtomicBool>,
    buffer: Vec<f32>,
    buffer_capacity: usize,
    channels: u16,
    sample_rate: u32,
}

impl<S: Source<Item = f32>> CaptureSource<S> {
    fn new(
        inner: S,
        sample_tx: Sender<Vec<f32>>,
        progress_counter: Arc<AtomicU64>,
        is_finished: Arc<AtomicBool>,
    ) -> Self {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();
        // Send visualizer data roughly every ~33ms (30fps)
        let buffer_capacity = (sample_rate as usize * channels as usize) / 30;

        Self {
            inner,
            sample_tx,
            progress_counter,
            is_finished,
            buffer: Vec::with_capacity(buffer_capacity),
            buffer_capacity,
            channels,
            sample_rate,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for CaptureSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self.inner.next() {
            Some(sample) => {
                self.progress_counter.fetch_add(1, Ordering::Relaxed);
                self.buffer.push(sample);

                if self.buffer.len() >= self.buffer_capacity {
                    // Downsample to mono for visualizer
                    let mono: Vec<f32> = if self.channels == 1 {
                        self.buffer.clone()
                    } else {
                        self.buffer
                            .chunks(self.channels as usize)
                            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
                            .collect()
                    };
                    let _ = self.sample_tx.try_send(mono);
                    self.buffer.clear();
                }

                Some(sample)
            }
            None => {
                self.is_finished.store(true, Ordering::Relaxed);
                None
            }
        }
    }
}

impl<S: Source<Item = f32>> Source for CaptureSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        // Reset progress counter to the seek position
        let sample_pos = (pos.as_secs_f64() * self.sample_rate as f64 * self.channels as f64) as u64;
        self.progress_counter.store(sample_pos, Ordering::Relaxed);
        self.inner.try_seek(pos)
    }
}

pub struct AudioEngine {
    cmd_rx: Receiver<AudioCommand>,
    event_tx: Sender<AudioEvent>,
    sample_tx: Sender<Vec<f32>>,
}

impl AudioEngine {
    pub fn new(
        cmd_rx: Receiver<AudioCommand>,
        event_tx: Sender<AudioEvent>,
        sample_tx: Sender<Vec<f32>>,
    ) -> Self {
        Self {
            cmd_rx,
            event_tx,
            sample_tx,
        }
    }

    pub fn run(self) {
        // Initialize audio output
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(AudioEvent::Error(format!("Failed to open audio output: {e}")));
                return;
            }
        };

        let sink = match Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(AudioEvent::Error(format!("Failed to create audio sink: {e}")));
                return;
            }
        };

        let progress_counter = Arc::new(AtomicU64::new(0));
        let is_finished = Arc::new(AtomicBool::new(false));
        let mut current_sample_rate: u32 = 44100;
        let mut current_channels: u16 = 2;
        let mut last_progress_send = std::time::Instant::now();

        loop {
            // Check for track finished
            if is_finished.load(Ordering::Relaxed) && sink.empty() {
                is_finished.store(false, Ordering::Relaxed);
                let _ = self.event_tx.send(AudioEvent::TrackFinished);
            }

            // Send progress updates at ~30fps
            if last_progress_send.elapsed() >= Duration::from_millis(33) {
                let samples = progress_counter.load(Ordering::Relaxed);
                let position = samples as f64 / (current_sample_rate as f64 * current_channels as f64);
                let _ = self.event_tx.send(AudioEvent::Progress(position));
                last_progress_send = std::time::Instant::now();
            }

            // Process commands (non-blocking with timeout)
            match self.cmd_rx.recv_timeout(Duration::from_millis(16)) {
                Ok(cmd) => match cmd {
                    AudioCommand::Play(path) => {
                        sink.stop();
                        progress_counter.store(0, Ordering::Relaxed);
                        is_finished.store(false, Ordering::Relaxed);

                        match Self::load_track(
                            &stream_handle,
                            &path,
                            self.sample_tx.clone(),
                            progress_counter.clone(),
                            is_finished.clone(),
                        ) {
                            Ok((new_sink, duration, sr, ch)) => {
                                // We need to replace sink - but sink is not mut.
                                // Instead, let's restructure to create a new sink each time.
                                // For now, use the returned sink.
                                current_sample_rate = sr;
                                current_channels = ch;
                                let _ = self.event_tx.send(AudioEvent::Playing { duration });

                                // We'll run a sub-loop for this track
                                self.run_track_playback(
                                    new_sink,
                                    &progress_counter,
                                    &is_finished,
                                    current_sample_rate,
                                    current_channels,
                                    &stream_handle,
                                );
                            }
                            Err(e) => {
                                let _ = self.event_tx.send(AudioEvent::Error(format!(
                                    "Failed to play {}: {e}",
                                    path.display()
                                )));
                            }
                        }
                    }
                    AudioCommand::Pause => sink.pause(),
                    AudioCommand::Resume => sink.play(),
                    AudioCommand::Stop => {
                        sink.stop();
                        progress_counter.store(0, Ordering::Relaxed);
                    }
                    AudioCommand::Seek(pos) => {
                        let _ = sink.try_seek(Duration::from_secs_f64(pos));
                    }
                    AudioCommand::SetVolume(vol) => {
                        sink.set_volume(vol);
                    }
                    AudioCommand::SetSpeed(speed) => {
                        sink.set_speed(speed);
                    }
                },
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn run_track_playback(
        &self,
        sink: Sink,
        progress_counter: &Arc<AtomicU64>,
        is_finished: &Arc<AtomicBool>,
        sample_rate: u32,
        channels: u16,
        stream_handle: &OutputStreamHandle,
    ) {
        let mut last_progress_send = std::time::Instant::now();

        loop {
            // Check for track finished
            if is_finished.load(Ordering::Relaxed) && sink.empty() {
                is_finished.store(false, Ordering::Relaxed);
                let _ = self.event_tx.send(AudioEvent::TrackFinished);
                return;
            }

            // Send progress updates at ~30fps
            if last_progress_send.elapsed() >= Duration::from_millis(33) {
                let samples = progress_counter.load(Ordering::Relaxed);
                let position = samples as f64 / (sample_rate as f64 * channels as f64);
                let _ = self.event_tx.send(AudioEvent::Progress(position));
                last_progress_send = std::time::Instant::now();
            }

            // Process commands
            match self.cmd_rx.recv_timeout(Duration::from_millis(16)) {
                Ok(cmd) => match cmd {
                    AudioCommand::Play(path) => {
                        sink.stop();
                        progress_counter.store(0, Ordering::Relaxed);
                        is_finished.store(false, Ordering::Relaxed);

                        match Self::load_track(
                            stream_handle,
                            &path,
                            self.sample_tx.clone(),
                            progress_counter.clone(),
                            is_finished.clone(),
                        ) {
                            Ok((new_sink, duration, sr, ch)) => {
                                let _ = self.event_tx.send(AudioEvent::Playing { duration });
                                // Recurse with the new sink
                                self.run_track_playback(
                                    new_sink,
                                    progress_counter,
                                    is_finished,
                                    sr,
                                    ch,
                                    stream_handle,
                                );
                                return;
                            }
                            Err(e) => {
                                let _ = self.event_tx.send(AudioEvent::Error(format!(
                                    "Failed to play: {e}"
                                )));
                                return;
                            }
                        }
                    }
                    AudioCommand::Pause => sink.pause(),
                    AudioCommand::Resume => sink.play(),
                    AudioCommand::Stop => {
                        sink.stop();
                        progress_counter.store(0, Ordering::Relaxed);
                        return;
                    }
                    AudioCommand::Seek(pos) => {
                        let seek_duration = Duration::from_secs_f64(pos.max(0.0));
                        let _ = sink.try_seek(seek_duration);
                        let sample_pos =
                            (pos * sample_rate as f64 * channels as f64) as u64;
                        progress_counter.store(sample_pos, Ordering::Relaxed);
                    }
                    AudioCommand::SetVolume(vol) => {
                        sink.set_volume(vol);
                    }
                    AudioCommand::SetSpeed(speed) => {
                        sink.set_speed(speed);
                    }
                },
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn load_track(
        stream_handle: &OutputStreamHandle,
        path: &std::path::Path,
        sample_tx: Sender<Vec<f32>>,
        progress_counter: Arc<AtomicU64>,
        is_finished: Arc<AtomicBool>,
    ) -> anyhow::Result<(Sink, f64, u32, u16)> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let decoder = Decoder::new(reader)?;

        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let total_duration = decoder
            .total_duration()
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        // Convert to f32 source
        let source = decoder.convert_samples::<f32>();

        // Wrap in capture source
        let capture = CaptureSource::new(source, sample_tx, progress_counter, is_finished);

        let sink = Sink::try_new(stream_handle)?;
        sink.append(capture);

        Ok((sink, total_duration, sample_rate, channels))
    }
}
