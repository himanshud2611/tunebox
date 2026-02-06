use image::{DynamicImage, GenericImageView, Rgba};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

const ART_WIDTH: u32 = 20;
const ART_HEIGHT: u32 = 20; // In terminal cells; each cell = 2 vertical pixels

pub struct AlbumArt {
    /// Each element is (top_pixel_color, bottom_pixel_color) for a half-block cell
    pixels: Vec<Vec<(Color, Color)>>,
    pub width: u16,
    pub height: u16,
}

impl AlbumArt {
    pub fn from_image_data(data: &[u8]) -> Option<Self> {
        let img = image::load_from_memory(data).ok()?;
        Some(Self::from_image(&img))
    }

    pub fn from_image(img: &DynamicImage) -> Self {
        // We need ART_WIDTH columns and ART_HEIGHT * 2 pixel rows
        // (each terminal cell holds 2 vertical pixels via half-blocks)
        let pixel_rows = ART_HEIGHT * 2;
        let resized = img.resize_exact(ART_WIDTH, pixel_rows, image::imageops::FilterType::Lanczos3);

        let mut pixels = Vec::with_capacity(ART_HEIGHT as usize);
        for row in 0..ART_HEIGHT {
            let mut row_pixels = Vec::with_capacity(ART_WIDTH as usize);
            for col in 0..ART_WIDTH {
                let top_y = row * 2;
                let bot_y = row * 2 + 1;
                let top = resized.get_pixel(col, top_y);
                let bot = resized.get_pixel(col, bot_y);
                row_pixels.push((rgba_to_color(top), rgba_to_color(bot)));
            }
            pixels.push(row_pixels);
        }

        Self {
            pixels,
            width: ART_WIDTH as u16,
            height: ART_HEIGHT as u16,
        }
    }

    pub fn placeholder() -> Self {
        // Create a music note icon with "tunebox" text - fits in 10 rows
        let bg = Color::Rgb(20, 28, 45);        // Dark background
        let accent = Color::Rgb(6, 182, 212);   // Cyan accent
        let accent2 = Color::Rgb(168, 85, 247); // Purple accent
        let text_color = Color::Rgb(200, 210, 220); // Light text

        // Compact design: note (rows 0-6) + text (rows 8-9) = 10 rows total
        #[rustfmt::skip]
        let note_pattern: [[u8; 20]; 7] = [
            [0,0,0,0,0,0,0,1,1,1,1,1,1,1,0,0,0,0,0,0], // beam
            [0,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,0], // stems
            [0,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,0], // stems
            [0,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,0], // stems
            [0,0,0,0,0,2,2,1,2,0,0,2,2,1,2,0,0,0,0,0], // note heads top
            [0,0,0,0,2,2,2,2,2,2,2,2,2,2,2,2,0,0,0,0], // note heads middle
            [0,0,0,0,0,2,2,2,2,0,0,2,2,2,2,0,0,0,0,0], // note heads bottom
        ];

        // "TUNEBOX" - compact pixel text centered
        #[rustfmt::skip]
        let text_pattern: [[u8; 20]; 3] = [
            [1,1,1,0,1,0,1,0,1,0,1,0,1,1,0,1,1,0,1,0], // T U N E B O X
            [0,1,0,0,1,0,1,0,1,1,1,0,1,0,0,1,0,1,0,1],
            [0,1,0,0,1,1,1,0,1,0,1,0,1,1,0,1,1,0,1,0],
        ];

        let mut pixels = Vec::with_capacity(ART_HEIGHT as usize);

        for row in 0..ART_HEIGHT as usize {
            let mut row_pixels = Vec::with_capacity(ART_WIDTH as usize);

            for col in 0..ART_WIDTH as usize {
                let (top_color, bot_color) = if row < 7 {
                    // Music note area
                    let pixel_val = note_pattern[row][col];
                    let color = match pixel_val {
                        1 => accent,
                        2 => accent2,
                        _ => bg,
                    };
                    (color, color)
                } else if row == 7 {
                    // Spacing
                    (bg, bg)
                } else if row >= 8 && row < 11 {
                    // Text area (rows 8, 9, 10)
                    let text_row = row - 8;
                    let pixel_val = text_pattern[text_row][col];
                    let color = if pixel_val == 1 { text_color } else { bg };
                    (color, color)
                } else {
                    (bg, bg)
                };

                row_pixels.push((top_color, bot_color));
            }
            pixels.push(row_pixels);
        }

        Self {
            pixels,
            width: ART_WIDTH as u16,
            height: ART_HEIGHT as u16,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let max_rows = area.height.min(self.height);
        let max_cols = area.width.min(self.width);

        for row in 0..max_rows as usize {
            if row >= self.pixels.len() {
                break;
            }
            for col in 0..max_cols as usize {
                if col >= self.pixels[row].len() {
                    break;
                }
                let (fg, bg) = self.pixels[row][col];
                let x = area.x + col as u16;
                let y = area.y + row as u16;
                if x < area.right() && y < area.bottom() {
                    let cell = &mut buf[(x, y)];
                    cell.set_char('▀');
                    cell.set_fg(fg);
                    cell.set_bg(bg);
                }
            }
        }
    }
}

fn rgba_to_color(pixel: Rgba<u8>) -> Color {
    Color::Rgb(pixel[0], pixel[1], pixel[2])
}
