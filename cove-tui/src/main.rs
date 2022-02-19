mod config;
mod never;
mod replies;
mod room;

use std::io::{self, Stdout};

use config::Config;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use palette::rgb::Rgb;
use palette::{FromColor, Hsl, Srgb};
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use tui::Terminal;

async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    terminal.draw(|f| {
        let hchunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(2),
                Constraint::Length(20),
            ])
            .split(f.size());

        // Borders
        f.render_widget(Block::default().borders(Borders::LEFT), hchunks[1]);
        f.render_widget(Block::default().borders(Borders::LEFT), hchunks[3]);

        // Room list
        let room_style = Style::default().fg(Color::LightBlue);
        let mut state = ListState::default();
        // state.select(Some(1));
        f.render_stateful_widget(
            List::new(vec![
                ListItem::new(Span::styled(
                    "Cove",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                ListItem::new(Span::styled("&dunno", room_style)),
                ListItem::new(Span::styled("&test", room_style)),
                ListItem::new(" "),
                ListItem::new(Span::styled(
                    "Euphoria",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                ListItem::new(Span::styled("&xkcd", room_style)),
                ListItem::new(Span::styled("&music", room_style)),
                ListItem::new(Span::styled("&bots", room_style)),
                ListItem::new(" "),
                ListItem::new(Span::styled(
                    "Instant",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                ListItem::new(Span::styled("&welcome", room_style)),
            ]),
            // .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            // .highlight_symbol(">"),
            hchunks[0],
            &mut state,
        );
        // f.render_widget(Paragraph::new("foo"), hchunks[0]);

        // Nick list
        let nchunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(hchunks[4]);
        f.render_widget(
            Paragraph::new(Spans::from(vec![
                Span::styled("Users", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled("(13)", Style::default().fg(Color::Gray)),
            ])),
            nchunks[0],
        );
        fn userstyle(r: u8, g: u8, b: u8) -> Style {
            let rgb = Srgb::new(r, g, b).into_format::<f32>();
            let mut hsl = Hsl::from_color(rgb);
            hsl.saturation = 1.0;
            hsl.lightness = 0.7;
            let rgb = Rgb::from_color(hsl).into_format::<u8>();
            Style::default().fg(Color::Rgb(rgb.red, rgb.green, rgb.blue))
        }
        f.render_widget(
            List::new([
                ListItem::new(Span::styled("TerryTvType", userstyle(192, 242, 238))),
                ListItem::new(Span::styled("r*4", userstyle(192, 211, 242))),
                ListItem::new(Span::styled("Swedish", userstyle(192, 242, 207))),
                ListItem::new(Span::styled("Garmy", userstyle(242, 225, 192))),
                ListItem::new(Span::styled("SRP", userstyle(242, 219, 192))),
                ListItem::new(Span::styled("C", userstyle(192, 218, 242))),
                ListItem::new(Span::styled("fill", userstyle(192, 197, 242))),
                ListItem::new(Span::styled("ohnezo", userstyle(242, 203, 192))),
                ListItem::new(Span::styled("Sumärzru", userstyle(242, 223, 192))),
                ListItem::new(Span::styled("SuperGeek", userstyle(192, 242, 203))),
                ListItem::new(Span::styled("certainlyhominid", userstyle(192, 242, 209))),
                ListItem::new(Span::styled("Plugh", userstyle(192, 242, 215))),
                ListItem::new(Span::styled(
                    "🎼\u{fe0e}🎷🎷🎷🎼\u{fe0e}",
                    userstyle(242, 192, 192),
                )),
            ]),
            nchunks[1],
        );
    })?;
    let _ = crossterm::event::read();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    crossterm::terminal::enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;

    // Defer error handling so the terminal always gets restored properly
    let result = run(&mut terminal).await;

    crossterm::terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    result?;

    Ok(())
}
