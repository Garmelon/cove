use crossterm::style::{Color, ContentStyle, Stylize};
use palette::{FromColor, Hsl, RgbHue, Srgb};

fn normalize(text: &str) -> String {
    // TODO Remove emoji names?
    text.chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A re-implementation of [euphoria's nick hue hashing algorithm][0].
///
/// [0]: https://github.com/euphoria-io/heim/blob/master/client/lib/hueHash.js
fn hue_hash(text: &str, offset: i64) -> u8 {
    let mut val = 0_i32;
    for char in text.chars() {
        let char_val = (char as i32).wrapping_mul(439) % 256;
        val = val.wrapping_mul(33).wrapping_add(char_val);
    }

    let val: i64 = val as i64 + 2_i64.pow(31);
    ((val + offset) % 255) as u8
}

const GREENIE_OFFSET: i64 = 148 - 192; // 148 - hue_hash("greenie", 0)

pub fn hue(text: &str) -> u8 {
    let normalized = normalize(text);
    if normalized.is_empty() {
        hue_hash(text, GREENIE_OFFSET)
    } else {
        hue_hash(&normalized, GREENIE_OFFSET)
    }
}

pub fn nick_color(nick: &str) -> (u8, u8, u8) {
    let hue = RgbHue::from(hue(nick) as f32);
    let color = Hsl::new(hue, 1.0, 0.72);
    Srgb::from_color(color)
        .into_format::<u8>()
        .into_components()
}

pub fn nick_style(nick: &str) -> ContentStyle {
    let (r, g, b) = nick_color(nick);
    ContentStyle::default().bold().with(Color::Rgb { r, g, b })
}
