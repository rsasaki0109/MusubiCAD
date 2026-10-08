//! Text captions on CPU preview images.
//!
//! A 5 × 7 bitmap font covering ASCII letters, digits, common punctuation, and
//! `° × → ±`, drawn at an integer pixel scale.  Captions label animation
//! frames with the values that produced them (a joint angle, a parameter), so
//! a GIF shared on its own still says what it shows.  Characters the font
//! lacks draw as a hollow box.

use crate::software::PreviewImage;

const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;
/// Horizontal advance per character, in font pixels (glyph plus one blank column).
const ADVANCE: u32 = GLYPH_WIDTH + 1;
/// Line advance, in font pixels.
const LINE_HEIGHT: u32 = GLYPH_HEIGHT + 3;
/// Caption box padding, in font pixels.
const PADDING: u32 = 3;

/// Rows of a glyph, top first; bit 4 is the leftmost column.
fn glyph(ch: char) -> [u8; 7] {
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        'a' => [
            0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111,
        ],
        'b' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110,
        ],
        'c' => [
            0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'd' => [
            0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110,
        ],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
        ],
        'g' => [
            0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'h' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'i' => [
            0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'j' => [
            0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001,
        ],
        'n' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'p' => [
            0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'q' => [
            0b00000, 0b00000, 0b01101, 0b10011, 0b01111, 0b00001, 0b00001,
        ],
        'r' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000,
        ],
        's' => [
            0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        't' => [
            0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110,
        ],
        'u' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101,
        ],
        'v' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'w' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010,
        ],
        'x' => [
            0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001,
        ],
        'y' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'z' => [
            0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        ' ' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        '/' => [
            0b00000, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '%' => [
            0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '\'' => [
            0b01100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '°' => [
            0b01100, 0b10010, 0b10010, 0b01100, 0b00000, 0b00000, 0b00000,
        ],
        '×' => [
            0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b00000,
        ],
        '→' => [
            0b00000, 0b00100, 0b00010, 0b11111, 0b00010, 0b00100, 0b00000,
        ],
        '±' => [
            0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000, 0b11111,
        ],
        _ => [
            0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
        ],
    }
}

/// Where a caption box sits on the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionCorner {
    TopLeft,
    BottomLeft,
}

/// Width and height in image pixels of `lines` drawn at `scale`, without the box.
pub fn text_size(lines: &[&str], scale: u32) -> (u32, u32) {
    let columns = lines
        .iter()
        .map(|line| line.chars().count() as u32)
        .max()
        .unwrap_or(0);
    let width = (columns * ADVANCE).saturating_sub(1) * scale;
    let height =
        (lines.len() as u32 * LINE_HEIGHT).saturating_sub(LINE_HEIGHT - GLYPH_HEIGHT) * scale;
    (width, height)
}

/// Image pixels a caption of `lines` at `scale` takes up from its edge of the
/// image, box and margin included; 0 for no lines.
pub fn caption_extent(lines: &[&str], scale: u32) -> u32 {
    if lines.is_empty() {
        return 0;
    }
    let scale = scale.max(1);
    let (_, text_height) = text_size(lines, scale);
    text_height + 4 * PADDING * scale
}

/// Draw `lines` in a translucent box in `corner`, `scale` image pixels per
/// font pixel.  Text that does not fit is clipped at the image edge.
pub fn draw_caption(image: &mut PreviewImage, lines: &[&str], corner: CaptionCorner, scale: u32) {
    let scale = scale.max(1);
    if lines.is_empty() {
        return;
    }
    let (text_width, text_height) = text_size(lines, scale);
    let pad = PADDING * scale;
    let margin = 2 * pad;
    let box_width = text_width + 2 * pad;
    let box_height = text_height + 2 * pad;
    let left = margin;
    let top = match corner {
        CaptionCorner::TopLeft => margin,
        CaptionCorner::BottomLeft => image.height.saturating_sub(margin + box_height),
    };
    blend_rect(image, left, top, box_width, box_height, [20, 24, 32], 0.72);
    for (row, line) in lines.iter().enumerate() {
        let y = top + pad + row as u32 * LINE_HEIGHT * scale;
        draw_text(image, left + pad, y, line, scale, [245, 247, 250]);
    }
}

/// Draw one line of text with its top-left corner at (`x`, `y`).
pub fn draw_text(image: &mut PreviewImage, x: u32, y: u32, text: &str, scale: u32, rgb: [u8; 3]) {
    let scale = scale.max(1);
    for (index, ch) in text.chars().enumerate() {
        let origin_x = x + index as u32 * ADVANCE * scale;
        for (row, bits) in glyph(ch).iter().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                    continue;
                }
                fill_rect(
                    image,
                    origin_x + column * scale,
                    y + row as u32 * scale,
                    scale,
                    scale,
                    rgb,
                );
            }
        }
    }
}

fn fill_rect(image: &mut PreviewImage, x: u32, y: u32, width: u32, height: u32, rgb: [u8; 3]) {
    blend_rect(image, x, y, width, height, rgb, 1.0);
}

fn blend_rect(
    image: &mut PreviewImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    rgb: [u8; 3],
    alpha: f32,
) {
    let x_end = x.saturating_add(width).min(image.width);
    let y_end = y.saturating_add(height).min(image.height);
    for py in y..y_end {
        for px in x..x_end {
            let index = ((py * image.width + px) * 4) as usize;
            for (value, over) in image.rgba[index..index + 3].iter_mut().zip(rgb) {
                let under = f32::from(*value);
                *value = (under + (f32::from(over) - under) * alpha).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(width: u32, height: u32) -> PreviewImage {
        PreviewImage {
            width,
            height,
            rgba: vec![255; (width * height * 4) as usize],
        }
    }

    fn lit(image: &PreviewImage, x: u32, y: u32) -> bool {
        image.rgba[((y * image.width + x) * 4) as usize] < 128
    }

    #[test]
    fn every_glyph_fits_five_columns_and_known_glyphs_differ_from_the_fallback() {
        let fallback = glyph('\u{1F916}');
        for ch in (' '..='~').chain(['°', '×', '→', '±']) {
            assert!(glyph(ch).iter().all(|row| *row < 32), "{ch}");
            let unsupported = "\"#$&*;@\\^`{|}~";
            if !unsupported.contains(ch) {
                assert_ne!(glyph(ch), fallback, "{ch} should have its own glyph");
            }
        }
    }

    #[test]
    fn text_is_drawn_at_scale_and_clipped_at_the_edge() {
        let mut image = blank(40, 20);
        draw_text(&mut image, 1, 1, "1", 2, [0, 0, 0]);
        // '1' has its stem in the middle column: font (2, 0) → pixels (5..7, 1..3).
        assert!(lit(&image, 5, 1) && lit(&image, 6, 2));
        assert!(!lit(&image, 1, 1));
        // Clipped, not panicking, past the right and bottom edges.
        draw_text(&mut image, 36, 16, "WWW", 3, [0, 0, 0]);
        assert_eq!(text_size(&["ab", "c"], 1), (11, 17));
        assert_eq!(caption_extent(&["ab", "c"], 1), 17 + 12);
        assert_eq!(caption_extent(&[], 2), 0);
    }

    #[test]
    fn captions_darken_a_box_in_the_chosen_corner() {
        let mut image = blank(200, 100);
        draw_caption(&mut image, &["elbow 84.0°"], CaptionCorner::BottomLeft, 1);
        assert!(lit(&image, 8, 100 - 8));
        assert!(!lit(&image, 8, 8));
        let mut top = blank(200, 100);
        draw_caption(&mut top, &["x"], CaptionCorner::TopLeft, 1);
        assert!(lit(&top, 8, 8));
    }
}
