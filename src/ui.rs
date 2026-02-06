use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, Theme};
use crate::visualizer::VisualizerMode;

// Theme color struct
pub struct ThemeColors {
    pub accent: Color,
    pub accent_secondary: Color,
    pub text_primary: Color,
    pub text_dim: Color,
    pub text_muted: Color,
    pub bg_dark: Color,
    pub bg_panel: Color,
    pub status_bg: Color,
}

impl ThemeColors {
    pub fn from_theme(theme: Theme) -> Self {
        match theme {
            Theme::Default => Self {
                accent: Color::Rgb(6, 182, 212),      // Cyan
                accent_secondary: Color::Rgb(168, 85, 247), // Magenta
                text_primary: Color::White,
                text_dim: Color::Rgb(148, 163, 184),
                text_muted: Color::Rgb(100, 116, 139),
                bg_dark: Color::Rgb(15, 23, 42),
                bg_panel: Color::Rgb(30, 41, 59),
                status_bg: Color::Rgb(51, 65, 85),
            },
            Theme::Dracula => Self {
                accent: Color::Rgb(139, 233, 253),    // Cyan
                accent_secondary: Color::Rgb(255, 121, 198), // Pink
                text_primary: Color::Rgb(248, 248, 242),
                text_dim: Color::Rgb(189, 147, 249),
                text_muted: Color::Rgb(98, 114, 164),
                bg_dark: Color::Rgb(40, 42, 54),
                bg_panel: Color::Rgb(68, 71, 90),
                status_bg: Color::Rgb(68, 71, 90),
            },
            Theme::Nord => Self {
                accent: Color::Rgb(136, 192, 208),    // Frost
                accent_secondary: Color::Rgb(180, 142, 173), // Purple
                text_primary: Color::Rgb(236, 239, 244),
                text_dim: Color::Rgb(216, 222, 233),
                text_muted: Color::Rgb(76, 86, 106),
                bg_dark: Color::Rgb(46, 52, 64),
                bg_panel: Color::Rgb(59, 66, 82),
                status_bg: Color::Rgb(67, 76, 94),
            },
            Theme::Gruvbox => Self {
                accent: Color::Rgb(215, 153, 33),     // Yellow
                accent_secondary: Color::Rgb(211, 134, 155), // Purple
                text_primary: Color::Rgb(235, 219, 178),
                text_dim: Color::Rgb(189, 174, 147),
                text_muted: Color::Rgb(146, 131, 116),
                bg_dark: Color::Rgb(40, 40, 40),
                bg_panel: Color::Rgb(60, 56, 54),
                status_bg: Color::Rgb(80, 73, 69),
            },
            Theme::Neon => Self {
                accent: Color::Rgb(0, 255, 136),      // Neon Green
                accent_secondary: Color::Rgb(255, 0, 128), // Hot Pink
                text_primary: Color::Rgb(255, 255, 255),
                text_dim: Color::Rgb(0, 255, 255),   // Cyan
                text_muted: Color::Rgb(128, 128, 128),
                bg_dark: Color::Rgb(0, 0, 0),
                bg_panel: Color::Rgb(20, 20, 30),
                status_bg: Color::Rgb(40, 0, 40),
            },
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();
    let colors = ThemeColors::from_theme(app.theme);

    // Mini mode - single line display
    if app.mini_mode {
        draw_mini_mode(frame, app, size, &colors);
        return;
    }

    // Main background
    let bg_block = Block::default().style(Style::default().bg(colors.bg_dark));
    frame.render_widget(bg_block, size);

    // Smart layout: give more space to visualizer when library is small
    let lib_size = app.filtered_indices.len();
    let vis_height = if lib_size <= 3 { 10 } else if lib_size <= 10 { 8 } else { 6 };
    let lib_min = if lib_size <= 3 { 4 } else { 6 };

    // Main layout: header area, visualizer, library, footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),           // Now Playing (art + info)
            Constraint::Length(vis_height),   // Visualizer (dynamic)
            Constraint::Min(lib_min),         // Library
            Constraint::Length(2),            // Footer (2 lines for help bar)
        ])
        .split(size);

    draw_now_playing(frame, app, main_chunks[0], &colors);
    draw_visualizer(frame, app, main_chunks[1], size.width, &colors);
    draw_library(frame, app, main_chunks[2], &colors);
    draw_footer(frame, app, main_chunks[3], &colors);

    // Search overlay
    if app.search_mode {
        draw_search(frame, app, size, &colors);
    }

    // Info panel overlay
    if app.show_info {
        draw_info_panel(frame, app, size, &colors);
    }
}

fn draw_mini_mode(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let buf = frame.buffer_mut();

    // Clear with background
    for x in area.x..area.right() {
        for y in area.y..area.bottom() {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_bg(colors.bg_dark);
        }
    }

    // Build single line: ▶ Title - Artist | 1:23/3:45 | Vol 80% | 1x | [SHUF] | Theme
    let track = app.current_track();
    let play_icon = if app.is_playing { "▶" } else { "■" };
    let title = track.map(|t| t.title.as_str()).unwrap_or("No track");
    let artist = track.map(|t| t.artist.as_str()).unwrap_or("");

    let elapsed = format_time(app.progress);
    let total = format_time(app.duration);
    let vol_pct = (app.volume * 100.0) as u32;

    let mut spans = vec![
        Span::styled(format!(" {} ", play_icon), Style::default().fg(colors.accent)),
        Span::styled(format!("{}", title), Style::default().fg(colors.text_primary).add_modifier(Modifier::BOLD)),
    ];

    if !artist.is_empty() {
        spans.push(Span::styled(format!(" - {}", artist), Style::default().fg(colors.text_dim)));
    }

    spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
    spans.push(Span::styled(format!("{}/{}", elapsed, total), Style::default().fg(colors.text_dim)));
    spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
    spans.push(Span::styled(format!("Vol {}%", vol_pct), Style::default().fg(colors.accent)));

    if app.speed != crate::app::PlaybackSpeed::Normal {
        spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
        spans.push(Span::styled(app.speed.label(), Style::default().fg(colors.accent_secondary)));
    }

    if app.shuffle {
        spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
        spans.push(Span::styled("SHUF", Style::default().fg(colors.accent)));
    }

    if let Some(remaining) = app.sleep_timer_remaining() {
        let mins = remaining.as_secs() / 60;
        spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
        spans.push(Span::styled(format!("Sleep {}m", mins), Style::default().fg(colors.accent_secondary)));
    }

    spans.push(Span::styled(" │ ", Style::default().fg(colors.text_muted)));
    spans.push(Span::styled(app.theme.name(), Style::default().fg(colors.text_muted)));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(colors.bg_dark));
    frame.render_widget(paragraph, Rect::new(area.x, area.y, area.width, 1));

    // Mini visualizer on second line if space
    if area.height > 1 {
        let vis_area = Rect::new(area.x, area.y + 1, area.width, 1);
        draw_mini_visualizer(frame, app, vis_area, colors);
    }
}

fn draw_mini_visualizer(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let buf = frame.buffer_mut();
    let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let width = area.width as usize;

    for (i, &bar_val) in app.visualizer.bars.iter().enumerate() {
        let x_pos = i * width / app.visualizer.bars.len().max(1);
        if x_pos >= width {
            break;
        }
        let x = area.x + x_pos as u16;
        let height_index = (bar_val * 7.0) as usize;
        let ch = bar_chars[height_index.min(7)];
        let t = i as f32 / app.visualizer.bars.len().max(1) as f32;
        let color = gradient_color_themed(t, colors);

        if x < area.right() {
            let cell = &mut buf[(x, area.y)];
            cell.set_char(ch);
            cell.set_fg(color);
            cell.set_bg(colors.bg_dark);
        }
    }
}

fn draw_now_playing(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(colors.text_muted))
        .style(Style::default().bg(colors.bg_dark));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into album art area and track info
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22), // Album art
            Constraint::Min(20),   // Track info
        ])
        .split(inner);

    // Draw album art
    let art_area = Rect {
        x: chunks[0].x + 1,
        y: chunks[0].y,
        width: chunks[0].width.saturating_sub(2).min(20),
        height: chunks[0].height.min(10),
    };
    if let Some(ref art) = app.album_art {
        art.render(art_area, frame.buffer_mut());
    }

    // Draw track info
    let info_area = chunks[1];
    draw_track_info(frame, app, info_area, colors);
}

fn draw_track_info(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    if area.height < 3 {
        return;
    }

    let track = app.current_track();

    let title = track
        .map(|t| t.title.as_str())
        .unwrap_or("No track playing");
    let artist_album = track
        .map(|t| format!("{} — {}", t.artist, t.album))
        .unwrap_or_default();

    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(colors.text_primary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            artist_album,
            Style::default().fg(colors.text_dim),
        )),
        Line::from(""),
    ];

    // Playback controls with styled status badges
    let play_icon = if app.is_playing { "▶" } else { "■" };

    let mut control_spans = vec![
        Span::styled("◄◄  ", Style::default().fg(colors.text_dim)),
        Span::styled(play_icon, Style::default().fg(if app.is_playing { colors.accent } else { colors.text_dim })),
        Span::styled("  ►►", Style::default().fg(colors.text_dim)),
    ];

    // Shuffle badge
    if app.shuffle {
        control_spans.push(Span::raw("  "));
        control_spans.push(Span::styled(
            " SHUF ",
            Style::default().fg(colors.accent).bg(colors.status_bg).add_modifier(Modifier::BOLD),
        ));
    }

    // Repeat badge
    match app.repeat {
        crate::app::RepeatMode::Off => {}
        crate::app::RepeatMode::All => {
            control_spans.push(Span::raw("  "));
            control_spans.push(Span::styled(
                " RPT:ALL ",
                Style::default().fg(colors.accent_secondary).bg(colors.status_bg).add_modifier(Modifier::BOLD),
            ));
        }
        crate::app::RepeatMode::One => {
            control_spans.push(Span::raw("  "));
            control_spans.push(Span::styled(
                " RPT:ONE ",
                Style::default().fg(colors.accent_secondary).bg(colors.status_bg).add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Speed indicator (if not normal)
    if app.speed != crate::app::PlaybackSpeed::Normal {
        control_spans.push(Span::raw("  "));
        control_spans.push(Span::styled(
            format!(" {} ", app.speed.label()),
            Style::default().fg(colors.accent_secondary).bg(colors.status_bg).add_modifier(Modifier::BOLD),
        ));
    }

    // Sleep timer indicator
    if let Some(remaining) = app.sleep_timer_remaining() {
        let mins = remaining.as_secs() / 60;
        let secs = remaining.as_secs() % 60;
        control_spans.push(Span::raw("  "));
        control_spans.push(Span::styled(
            format!(" SLEEP {}:{:02} ", mins, secs),
            Style::default().fg(colors.accent).bg(colors.status_bg).add_modifier(Modifier::BOLD),
        ));
    }

    // Volume indicator
    let vol_pct = (app.volume * 100.0) as u32;
    let vol_bars = (app.volume * 8.0) as usize;
    control_spans.push(Span::raw("   "));
    control_spans.push(Span::styled("Vol ", Style::default().fg(colors.text_muted)));
    control_spans.push(Span::styled(
        "█".repeat(vol_bars),
        Style::default().fg(colors.accent),
    ));
    control_spans.push(Span::styled(
        "░".repeat(8usize.saturating_sub(vol_bars)),
        Style::default().fg(colors.text_muted),
    ));
    control_spans.push(Span::styled(
        format!(" {}%", vol_pct),
        Style::default().fg(colors.text_dim),
    ));

    lines.push(Line::from(control_spans));

    // Progress bar with gradient and remaining time
    if app.duration > 0.0 {
        let elapsed = format_time(app.progress);
        let remaining = format_time(app.duration - app.progress);
        let total = format_time(app.duration);
        let bar_width = area.width.saturating_sub(22) as usize;
        let progress_ratio = (app.progress / app.duration).clamp(0.0, 1.0);
        let filled = (progress_ratio * bar_width as f64) as usize;
        let empty = bar_width.saturating_sub(filled);

        lines.push(Line::from(""));

        let mut progress_spans = vec![
            Span::styled(elapsed, Style::default().fg(colors.text_dim)),
            Span::raw(" "),
        ];

        for i in 0..filled {
            let t = i as f32 / bar_width.max(1) as f32;
            let color = gradient_color_themed(t, colors);
            progress_spans.push(Span::styled("━", Style::default().fg(color)));
        }

        progress_spans.push(Span::styled("●", Style::default().fg(colors.text_primary)));
        progress_spans.push(Span::styled("─".repeat(empty), Style::default().fg(colors.text_muted)));
        progress_spans.push(Span::raw(" "));
        progress_spans.push(Span::styled(total, Style::default().fg(colors.text_dim)));
        progress_spans.push(Span::styled(format!("  -{}", remaining), Style::default().fg(colors.text_muted)));

        lines.push(Line::from(progress_spans));
    }

    if let Some(ref err) = app.error_message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )));
    }

    let paragraph = Paragraph::new(lines).style(Style::default().bg(colors.bg_dark));
    frame.render_widget(paragraph, area);
}

fn draw_visualizer(frame: &mut Frame, app: &App, area: Rect, _terminal_width: u16, colors: &ThemeColors) {
    // Draw a subtle border/separator at top
    let buf = frame.buffer_mut();
    for x in area.x..area.right() {
        if area.y > 0 {
            let cell = &mut buf[(x, area.y)];
            cell.set_char('─');
            cell.set_fg(colors.text_muted);
            cell.set_bg(colors.bg_dark);
        }
    }

    let inner_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    match app.visualizer.mode {
        VisualizerMode::FrequencyBars => draw_frequency_bars(frame, app, inner_area, colors),
        VisualizerMode::Waveform => draw_waveform(frame, app, inner_area, colors),
        VisualizerMode::Off => {
            let block = Block::default().style(Style::default().bg(colors.bg_dark));
            frame.render_widget(block, inner_area);
        }
    }
}

fn draw_frequency_bars(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let buf = frame.buffer_mut();
    let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let height = area.height as usize;

    if height == 0 {
        return;
    }

    let num_bars = app.visualizer.bars.len().min(area.width as usize);
    let bar_width = if num_bars > 0 {
        (area.width as usize / num_bars).max(1)
    } else {
        1
    };

    // Clear the area first
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_bg(colors.bg_dark);
        }
    }

    for (i, &bar_val) in app.visualizer.bars.iter().enumerate().take(num_bars) {
        // Scale bar value to full height
        let bar_height = (bar_val * height as f32 * 8.0) as usize; // 8 levels per character
        let full_blocks = bar_height / 8;
        let partial = bar_height % 8;

        // Color gradient using theme colors
        let t = i as f32 / num_bars.max(1) as f32;
        let color = gradient_color_themed(t, colors);

        let x_start = area.x + (i * bar_width) as u16;

        // Draw from bottom up
        for row in 0..height {
            let y = area.y + (height - 1 - row) as u16;
            if y >= area.bottom() {
                continue;
            }

            let ch = if row < full_blocks {
                bar_chars[7] // Full block
            } else if row == full_blocks && partial > 0 {
                bar_chars[partial - 1] // Partial block
            } else {
                ' '
            };

            if ch != ' ' {
                for dx in 0..bar_width.min((area.right() - x_start) as usize) {
                    let x = x_start + dx as u16;
                    if x < area.right() {
                        let cell = &mut buf[(x, y)];
                        cell.set_char(ch);
                        cell.set_fg(color);
                        cell.set_bg(colors.bg_dark);
                    }
                }
            }
        }
    }
}

fn draw_waveform(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let buf = frame.buffer_mut();
    let width = area.width as usize;
    let height = area.height as usize;

    if width == 0 || height == 0 {
        return;
    }

    // Clear the area first
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_bg(colors.bg_dark);
        }
    }

    // Draw center line
    let center_y = area.y + (height / 2) as u16;
    for x in area.x..area.right() {
        if center_y < area.bottom() {
            let cell = &mut buf[(x, center_y)];
            cell.set_char('─');
            cell.set_fg(colors.text_muted);
            cell.set_bg(colors.bg_dark);
        }
    }

    // Draw waveform with filled areas
    let mut prev_y: Option<i32> = None;
    for x_offset in 0..width {
        let idx = x_offset * app.visualizer.waveform.len() / width.max(1);
        if idx >= app.visualizer.waveform.len() {
            continue;
        }

        let val = app.visualizer.waveform[idx].clamp(-1.0, 1.0);
        // Map -1..1 to 0..height
        let y_pos = ((1.0 - val) * 0.5 * (height as f32 - 1.0)) as i32;
        let center = (height / 2) as i32;

        let x = area.x + x_offset as u16;
        let t = x_offset as f32 / width.max(1) as f32;
        let color = gradient_color_themed(t, colors);

        // Draw filled area from center to value
        let (start_y, end_y) = if y_pos < center {
            (y_pos, center)
        } else {
            (center, y_pos)
        };

        for y_idx in start_y..=end_y {
            let y = area.y + y_idx.clamp(0, height as i32 - 1) as u16;
            if x < area.right() && y < area.bottom() {
                let cell = &mut buf[(x, y)];
                if y_idx == y_pos {
                    cell.set_char('█');
                } else {
                    cell.set_char('▒');
                }
                cell.set_fg(color);
                cell.set_bg(colors.bg_dark);
            }
        }

        // Connect to previous point for smoother lines
        if let Some(prev) = prev_y {
            let step = if y_pos > prev { 1 } else { -1 };
            let mut cy = prev;
            while cy != y_pos {
                cy += step;
                let y = area.y + cy.clamp(0, height as i32 - 1) as u16;
                if x < area.right() && y < area.bottom() {
                    let cell = &mut buf[(x, y)];
                    if cell.symbol() == " " {
                        cell.set_char('│');
                        cell.set_fg(color);
                        cell.set_bg(colors.bg_dark);
                    }
                }
            }
        }

        prev_y = Some(y_pos);
    }
}

fn draw_library(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    // Show track count in title
    let track_count = app.filtered_indices.len();
    let title = format!(" Library ({}) ", track_count);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(colors.text_primary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.text_muted))
        .style(Style::default().bg(colors.bg_panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.filtered_indices.is_empty() {
        let msg = if app.search_mode {
            "No matches found"
        } else {
            "No audio files found"
        };
        let paragraph = Paragraph::new(Span::styled(msg, Style::default().fg(colors.text_muted)));
        frame.render_widget(paragraph, inner);
        return;
    }

    let visible_height = inner.height as usize;

    // Adjust scroll offset to keep selected visible
    let scroll = calculate_scroll(app.selected_index, visible_height, app.scroll_offset);

    // Responsive column widths based on terminal width
    let available_width = inner.width.saturating_sub(10) as usize; // 10 for indicator + duration + spacing
    let title_width = (available_width * 50 / 100).max(15).min(50);
    let artist_width = (available_width * 30 / 100).max(10).min(30);
    let album_width = available_width.saturating_sub(title_width + artist_width).max(0).min(25);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(display_idx, &lib_idx)| {
            let track = &app.library[lib_idx];
            let is_playing = app.playing_index == Some(lib_idx);
            let is_selected = display_idx == app.selected_index;

            // Animated playing indicator
            let indicator = if is_playing { "▶ " } else { "  " };
            let duration_str = format_time(track.duration);

            let title_style = if is_playing {
                Style::default().fg(colors.accent).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(colors.text_primary)
            } else {
                Style::default().fg(colors.text_dim)
            };

            let mut spans = vec![
                Span::styled(
                    indicator,
                    Style::default().fg(if is_playing { colors.accent } else { colors.text_muted }),
                ),
                Span::styled(
                    truncate_str(&track.title, title_width),
                    title_style,
                ),
                Span::styled(" ", Style::default()),
                Span::styled(
                    truncate_str(&track.artist, artist_width),
                    Style::default().fg(colors.text_muted),
                ),
            ];

            // Add album if there's space
            if album_width > 5 {
                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    truncate_str(&track.album, album_width),
                    Style::default().fg(colors.text_muted),
                ));
            }

            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(duration_str, Style::default().fg(colors.text_muted)));

            let line = Line::from(spans);

            let bg = if is_selected {
                colors.status_bg
            } else {
                colors.bg_panel
            };

            ListItem::new(line).style(Style::default().bg(bg))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    if area.height < 2 {
        return;
    }

    // Split footer into two rows
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // First row: Track info + theme name
    let track_info = if let Some(track) = app.current_track() {
        let bitrate = track
            .bitrate
            .map(|b| format!("{}kbps", b))
            .unwrap_or_default();
        let sample_rate = track
            .sample_rate
            .map(|s| format!("{:.1}kHz", s as f32 / 1000.0))
            .unwrap_or_default();
        format!("{}  {}  {}", bitrate, track.format, sample_rate)
    } else {
        String::new()
    };

    let vis_mode = format!("Vis: {}", app.visualizer.mode.label());
    let theme_name = format!("Theme: {}", app.theme.name());

    let info_line = Line::from(vec![
        Span::styled(track_info, Style::default().fg(colors.text_muted)),
        Span::styled("     ", Style::default()),
        Span::styled(vis_mode, Style::default().fg(colors.text_dim)),
        Span::styled("  │  ", Style::default().fg(colors.text_muted)),
        Span::styled(theme_name, Style::default().fg(colors.accent)),
    ]);

    let info_paragraph = Paragraph::new(info_line).style(Style::default().bg(colors.bg_dark));
    frame.render_widget(info_paragraph, footer_chunks[0]);

    // Second row: Help bar with keyboard shortcuts (including new keys)
    let help_spans = vec![
        Span::styled(" Space", Style::default().fg(colors.accent)),
        Span::styled(" Play  ", Style::default().fg(colors.text_muted)),
        Span::styled("n/p", Style::default().fg(colors.accent)),
        Span::styled(" Next/Prev  ", Style::default().fg(colors.text_muted)),
        Span::styled("s", Style::default().fg(colors.accent)),
        Span::styled(" Shuf  ", Style::default().fg(colors.text_muted)),
        Span::styled("v", Style::default().fg(colors.accent)),
        Span::styled(" Vis  ", Style::default().fg(colors.text_muted)),
        Span::styled("T", Style::default().fg(colors.accent)),
        Span::styled(" Theme  ", Style::default().fg(colors.text_muted)),
        Span::styled("t", Style::default().fg(colors.accent)),
        Span::styled(" Sleep  ", Style::default().fg(colors.text_muted)),
        Span::styled("</>", Style::default().fg(colors.accent)),
        Span::styled(" Speed  ", Style::default().fg(colors.text_muted)),
        Span::styled("m", Style::default().fg(colors.accent)),
        Span::styled(" Mini  ", Style::default().fg(colors.text_muted)),
        Span::styled("q", Style::default().fg(colors.accent)),
        Span::styled(" Quit", Style::default().fg(colors.text_muted)),
    ];

    let help_line = Line::from(help_spans);
    let help_paragraph = Paragraph::new(help_line).style(Style::default().bg(colors.bg_dark));
    frame.render_widget(help_paragraph, footer_chunks[1]);
}

fn draw_search(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let width = area.width.min(50);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.bottom().saturating_sub(4);

    let search_area = Rect::new(x, y, width, 3);
    frame.render_widget(Clear, search_area);

    let block = Block::default()
        .title(Span::styled(" Search ", Style::default().fg(colors.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.accent))
        .style(Style::default().bg(colors.bg_panel));

    let inner = block.inner(search_area);
    frame.render_widget(block, search_area);

    let search_text = format!("/{}", app.search_query);
    let paragraph = Paragraph::new(Span::styled(
        search_text,
        Style::default().fg(colors.text_primary),
    ));
    frame.render_widget(paragraph, inner);
}

fn draw_info_panel(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let track = match app.current_track() {
        Some(t) => t,
        None => return,
    };

    let width = area.width.min(60);
    let height = area.height.min(14);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let panel_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, panel_area);

    let block = Block::default()
        .title(Span::styled(
            " Track Info ",
            Style::default().fg(colors.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(colors.accent))
        .style(Style::default().bg(colors.bg_panel));

    let inner = block.inner(panel_area);
    frame.render_widget(block, panel_area);

    let info_lines = vec![
        Line::from(vec![
            Span::styled("Title:       ", Style::default().fg(colors.text_muted)),
            Span::styled(&track.title, Style::default().fg(colors.text_primary)),
        ]),
        Line::from(vec![
            Span::styled("Artist:      ", Style::default().fg(colors.text_muted)),
            Span::styled(&track.artist, Style::default().fg(colors.text_primary)),
        ]),
        Line::from(vec![
            Span::styled("Album:       ", Style::default().fg(colors.text_muted)),
            Span::styled(&track.album, Style::default().fg(colors.text_primary)),
        ]),
        Line::from(vec![
            Span::styled("Duration:    ", Style::default().fg(colors.text_muted)),
            Span::styled(
                format_time(track.duration),
                Style::default().fg(colors.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Format:      ", Style::default().fg(colors.text_muted)),
            Span::styled(&track.format, Style::default().fg(colors.text_primary)),
        ]),
        Line::from(vec![
            Span::styled("Bitrate:     ", Style::default().fg(colors.text_muted)),
            Span::styled(
                track
                    .bitrate
                    .map(|b| format!("{} kbps", b))
                    .unwrap_or_else(|| "N/A".to_string()),
                Style::default().fg(colors.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Sample Rate: ", Style::default().fg(colors.text_muted)),
            Span::styled(
                track
                    .sample_rate
                    .map(|s| format!("{} Hz", s))
                    .unwrap_or_else(|| "N/A".to_string()),
                Style::default().fg(colors.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("File Size:   ", Style::default().fg(colors.text_muted)),
            Span::styled(
                format_file_size(track.file_size),
                Style::default().fg(colors.text_primary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Path:        ", Style::default().fg(colors.text_muted)),
            Span::styled(
                track.path.display().to_string(),
                Style::default().fg(colors.text_dim),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(info_lines);
    frame.render_widget(paragraph, inner);
}

fn gradient_color(t: f32) -> Color {
    // Cyan (#06B6D4) -> Blue (#3B82F6) -> Magenta (#A855F7)
    let (r, g, b) = if t < 0.5 {
        let t2 = t * 2.0;
        (
            lerp(6.0, 59.0, t2) as u8,
            lerp(182.0, 130.0, t2) as u8,
            lerp(212.0, 246.0, t2) as u8,
        )
    } else {
        let t2 = (t - 0.5) * 2.0;
        (
            lerp(59.0, 168.0, t2) as u8,
            lerp(130.0, 85.0, t2) as u8,
            lerp(246.0, 247.0, t2) as u8,
        )
    };
    Color::Rgb(r, g, b)
}

fn gradient_color_themed(t: f32, colors: &ThemeColors) -> Color {
    // Interpolate between accent and accent_secondary based on position
    let (ar, ag, ab) = match colors.accent {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => (6.0, 182.0, 212.0),
    };
    let (sr, sg, sb) = match colors.accent_secondary {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => (168.0, 85.0, 247.0),
    };

    let r = lerp(ar, sr, t) as u8;
    let g = lerp(ag, sg, t) as u8;
    let b = lerp(ab, sb, t) as u8;

    Color::Rgb(r, g, b)
}

fn draw_stereo_spectrum(frame: &mut Frame, app: &App, area: Rect, colors: &ThemeColors) {
    let buf = frame.buffer_mut();
    let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let height = area.height as usize;
    let width = area.width as usize;

    if height == 0 || width == 0 {
        return;
    }

    // Clear the area first
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_bg(colors.bg_dark);
        }
    }

    // Draw center divider
    let center_x = area.x + (width / 2) as u16;
    for y in area.y..area.bottom() {
        if center_x < area.right() {
            let cell = &mut buf[(center_x, y)];
            cell.set_char('│');
            cell.set_fg(colors.text_muted);
            cell.set_bg(colors.bg_dark);
        }
    }

    // Left channel (bars grow left from center)
    let left_width = width / 2;
    let num_left_bars = app.visualizer.left_bars.len().min(left_width);
    if num_left_bars > 0 {
        let bar_width = (left_width / num_left_bars).max(1);

        for (i, &bar_val) in app.visualizer.left_bars.iter().enumerate().take(num_left_bars) {
            let bar_height = (bar_val * height as f32 * 8.0) as usize;
            let full_blocks = bar_height / 8;
            let partial = bar_height % 8;

            let t = i as f32 / num_left_bars.max(1) as f32;
            let color = gradient_color_themed(t, colors);

            // Draw from right to left (mirrored)
            let x_start = center_x.saturating_sub(1) - (i * bar_width) as u16;

            for row in 0..height {
                let y = area.y + (height - 1 - row) as u16;
                if y >= area.bottom() {
                    continue;
                }

                let ch = if row < full_blocks {
                    bar_chars[7]
                } else if row == full_blocks && partial > 0 {
                    bar_chars[partial - 1]
                } else {
                    ' '
                };

                if ch != ' ' {
                    for dx in 0..bar_width.min((x_start.saturating_sub(area.x) + 1) as usize) {
                        let x = x_start.saturating_sub(dx as u16);
                        if x >= area.x && x < center_x {
                            let cell = &mut buf[(x, y)];
                            cell.set_char(ch);
                            cell.set_fg(color);
                            cell.set_bg(colors.bg_dark);
                        }
                    }
                }
            }
        }
    }

    // Right channel (bars grow right from center)
    let right_width = width - width / 2 - 1;
    let num_right_bars = app.visualizer.right_bars.len().min(right_width);
    if num_right_bars > 0 {
        let bar_width = (right_width / num_right_bars).max(1);

        for (i, &bar_val) in app.visualizer.right_bars.iter().enumerate().take(num_right_bars) {
            let bar_height = (bar_val * height as f32 * 8.0) as usize;
            let full_blocks = bar_height / 8;
            let partial = bar_height % 8;

            let t = i as f32 / num_right_bars.max(1) as f32;
            let color = gradient_color_themed(t, colors);

            let x_start = center_x + 1 + (i * bar_width) as u16;

            for row in 0..height {
                let y = area.y + (height - 1 - row) as u16;
                if y >= area.bottom() {
                    continue;
                }

                let ch = if row < full_blocks {
                    bar_chars[7]
                } else if row == full_blocks && partial > 0 {
                    bar_chars[partial - 1]
                } else {
                    ' '
                };

                if ch != ' ' {
                    for dx in 0..bar_width.min((area.right() - x_start) as usize) {
                        let x = x_start + dx as u16;
                        if x < area.right() {
                            let cell = &mut buf[(x, y)];
                            cell.set_char(ch);
                            cell.set_fg(color);
                            cell.set_bg(colors.bg_dark);
                        }
                    }
                }
            }
        }
    }

    // Draw L/R labels
    if area.y < area.bottom() {
        let left_label_x = area.x + 1;
        let right_label_x = area.right().saturating_sub(2);

        if left_label_x < center_x {
            let cell = &mut buf[(left_label_x, area.y)];
            cell.set_char('L');
            cell.set_fg(colors.accent);
            cell.set_bg(colors.bg_dark);
        }
        if right_label_x > center_x && right_label_x < area.right() {
            let cell = &mut buf[(right_label_x, area.y)];
            cell.set_char('R');
            cell.set_fg(colors.accent_secondary);
            cell.set_bg(colors.bg_dark);
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn format_time(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{}:{:02}", m, s)
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

fn calculate_scroll(selected: usize, visible: usize, current_scroll: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if selected < current_scroll {
        selected
    } else if selected >= current_scroll + visible {
        selected - visible + 1
    } else {
        current_scroll
    }
}
