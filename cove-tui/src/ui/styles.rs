use tui::style::{Color, Modifier, Style};

pub fn title() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn error()->Style{
    Style::default().fg(Color::Red)
}

pub fn room() -> Style {
    Style::default().fg(Color::LightBlue)
}

pub fn selected_room() -> Style {
    room().add_modifier(Modifier::BOLD)
}
