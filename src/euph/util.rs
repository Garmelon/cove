use crossterm::style::{Color, ContentStyle, Stylize};
use palette::{FromColor, Hsl, RgbHue, Srgb};

pub fn nick_color(nick: &str) -> (u8, u8, u8) {
    let hue = RgbHue::from(euphoxide::nick_hue(nick) as f32);
    let color = Hsl::new(hue, 1.0, 0.72);
    Srgb::from_color(color)
        .into_format::<u8>()
        .into_components()
}

pub fn nick_style(nick: &str) -> ContentStyle {
    let (r, g, b) = nick_color(nick);
    ContentStyle::default().bold().with(Color::Rgb { r, g, b })
}
