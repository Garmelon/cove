use std::sync::LazyLock;

use crossterm::style::{Color, Stylize};
use euphoxide::Emoji;
use toss::{Style, Styled};

pub static EMOJI: LazyLock<Emoji> = LazyLock::new(Emoji::load);

/// Convert HSL to RGB following [this approach from wikipedia][1].
///
/// `h` must be in the range `[0, 360]`, `s` and `l` in the range `[0, 1]`.
///
/// [1]: https://en.wikipedia.org/wiki/HSL_and_HSV#HSL_to_RGB
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    assert!((0.0..=360.0).contains(&h), "h must be in range [0, 360]");
    assert!((0.0..=1.0).contains(&s), "s must be in range [0, 1]");
    assert!((0.0..=1.0).contains(&l), "l must be in range [0, 1]");

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;

    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());

    let (r1, g1, b1) = match () {
        _ if h_prime < 1.0 => (c, x, 0.0),
        _ if h_prime < 2.0 => (x, c, 0.0),
        _ if h_prime < 3.0 => (0.0, c, x),
        _ if h_prime < 4.0 => (0.0, x, c),
        _ if h_prime < 5.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let m = l - c / 2.0;
    let (r, g, b) = (r1 + m, g1 + m, b1 + m);

    // The rgb values in the range [0,1] are each split into 256 segments of the
    // same length, which are then assigned to the 256 possible values of an u8.
    ((r * 256.0) as u8, (g * 256.0) as u8, (b * 256.0) as u8)
}

pub fn nick_color(nick: &str) -> (u8, u8, u8) {
    let hue = euphoxide::nick::hue(&EMOJI, nick) as f32;
    hsl_to_rgb(hue, 1.0, 0.72)
}

pub fn nick_style(nick: &str, base: Style) -> Style {
    let (r, g, b) = nick_color(nick);
    base.bold().with(Color::Rgb { r, g, b })
}

pub fn style_nick(nick: &str, base: Style) -> Styled {
    Styled::new(EMOJI.replace(nick), nick_style(nick, base))
}

pub fn style_nick_exact(nick: &str, base: Style) -> Styled {
    Styled::new(nick, nick_style(nick, base))
}

pub fn style_mention(mention: &str, base: Style) -> Styled {
    let nick = mention
        .strip_prefix('@')
        .expect("mention must start with @");
    Styled::new(EMOJI.replace(mention), nick_style(nick, base))
}

pub fn style_mention_exact(mention: &str, base: Style) -> Styled {
    let nick = mention
        .strip_prefix('@')
        .expect("mention must start with @");
    Styled::new(mention, nick_style(nick, base))
}
