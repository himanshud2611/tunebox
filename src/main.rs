mod albumart;
mod app;
mod audio;
mod library;
mod metadata;
mod remote;
mod ui;
mod visualizer;

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use crossbeam_channel::bounded;

use app::PlaybackState;
use remote::{RemoteCommand, RemoteServer};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;
use audio::{AudioCommand, AudioEngine};

#[derive(Parser)]
#[command(
    name = "tunebox",
    about = "A beautiful terminal music player",
    version
)]
struct Cli {
    /// Path to a music directory or file
    path: PathBuf,

    /// Start with shuffle enabled
    #[arg(long)]
    shuffle: bool,

    /// Port for remote control server (default: 8080)
    #[arg(long, default_value = "8080")]
    port: u16,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli.path.canonicalize().context("Invalid path")?;

    // Scan library
    let tracks = if path.is_file() {
        library::scan_single_file(&path)?
    } else if path.is_dir() {
        library::scan_directory(&path)?
    } else {
        bail!("Path is neither a file nor directory: {}", path.display());
    };

    if tracks.is_empty() {
        bail!("No audio files found in {}", path.display());
    }

    eprintln!("Found {} tracks. Starting tunebox...", tracks.len());

    // Create communication channels
    let (cmd_tx, cmd_rx) = bounded::<AudioCommand>(32);
    let (event_tx, event_rx) = bounded(64);
    let (sample_tx, sample_rx) = bounded(4);

    // Create shared state for remote control
    let playback_state = Arc::new(Mutex::new(PlaybackState::default()));
    let (remote_cmd_tx, remote_cmd_rx) = bounded::<RemoteCommand>(32);

    // Start audio engine in a separate thread
    let audio_engine = AudioEngine::new(cmd_rx, event_tx, sample_tx);
    std::thread::spawn(move || {
        audio_engine.run();
    });

    // Start remote control server
    let remote_state = playback_state.clone();
    let remote_port = cli.port;
    std::thread::spawn(move || {
        let server = RemoteServer::new(remote_state, remote_cmd_tx);
        server.run(remote_port);
    });

    // Print remote control URL
    if let Some(ip) = remote::get_local_ip() {
        eprintln!("Remote control: http://{}:{}", ip, cli.port);
    } else {
        eprintln!("Remote control: http://localhost:{}", cli.port);
    }

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(tracks, cmd_tx, event_rx, sample_rx);

    if cli.shuffle {
        app.toggle_shuffle();
    }

    // If a single file was passed, start playing immediately
    if path.is_file() {
        app.play_track(0);
    }

    // Main event loop
    let result = run_app(&mut terminal, &mut app, playback_state, remote_cmd_rx);

    // Restore terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    playback_state: Arc<Mutex<PlaybackState>>,
    remote_cmd_rx: crossbeam_channel::Receiver<RemoteCommand>,
) -> Result<()> {
    loop {
        // Process audio events and samples
        app.process_audio_events();

        // Process remote commands
        while let Ok(cmd) = remote_cmd_rx.try_recv() {
            match cmd {
                RemoteCommand::Toggle => app.toggle_pause(),
                RemoteCommand::Next => app.next_track(),
                RemoteCommand::Prev => app.prev_track(),
                RemoteCommand::SetVolume(v) => {
                    app.volume = v;
                    let _ = app.cmd_tx.send(AudioCommand::SetVolume(v));
                }
                RemoteCommand::Seek(t) => {
                    let _ = app.cmd_tx.send(AudioCommand::Seek(t));
                }
                RemoteCommand::CycleTheme => app.cycle_theme(),
                RemoteCommand::CycleVisualizer => {
                    app.visualizer.mode = app.visualizer.mode.cycle();
                }
                RemoteCommand::ToggleShuffle => app.toggle_shuffle(),
            }
        }

        // Update shared playback state for remote
        if let Ok(mut state) = playback_state.try_lock() {
            *state = app.playback_state();
        }

        // Update sleep timer (fade volume, auto-pause)
        app.update_sleep_timer();

        // Draw
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Handle input with timeout for ~30fps rendering
        if event::poll(Duration::from_millis(33))? {
            if let Event::Key(key) = event::read()? {
                if app.search_mode {
                    handle_search_input(app, key.code);
                } else {
                    handle_normal_input(app, key.code, key.modifiers);
                }
            }
        }

        if app.should_quit {
            let _ = app.cmd_tx.send(AudioCommand::Stop);
            break;
        }
    }

    Ok(())
}

fn handle_normal_input(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char(' ') => app.toggle_pause(),
        KeyCode::Char('n') => app.next_track(),
        KeyCode::Char('p') => app.prev_track(),
        KeyCode::Char('j') | KeyCode::Down => app.move_selection_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_selection_up(),
        KeyCode::Enter => app.play_selected(),
        KeyCode::Char('s') => app.toggle_shuffle(),
        KeyCode::Char('r') => app.cycle_repeat(),
        KeyCode::Char('+') | KeyCode::Char(']') => app.volume_up(),
        KeyCode::Char('-') | KeyCode::Char('[') => app.volume_down(),
        KeyCode::Char('/') => app.toggle_search(),
        KeyCode::Right => app.seek_forward(),
        KeyCode::Left => app.seek_backward(),
        KeyCode::Char('i') => app.show_info = !app.show_info,
        KeyCode::Char('v') => {
            app.visualizer.mode = app.visualizer.mode.cycle();
        }
        // New features
        KeyCode::Char('T') => app.cycle_theme(),
        KeyCode::Char('t') => app.cycle_sleep_timer(),
        KeyCode::Char('m') => app.toggle_mini_mode(),
        KeyCode::Char('<') | KeyCode::Char(',') => app.speed_down(),
        KeyCode::Char('>') | KeyCode::Char('.') => app.speed_up(),
        _ => {}
    }
}

fn handle_search_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Enter => app.toggle_search(),
        KeyCode::Backspace => app.search_backspace(),
        KeyCode::Char(c) => app.search_input(c),
        _ => {}
    }
}
