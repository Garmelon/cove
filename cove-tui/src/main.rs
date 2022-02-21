mod config;
mod never;
mod replies;
mod room;
mod textline;

use std::io::{self, Stdout};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use textline::{TextLine, TextLineState};
use tui::backend::{Backend, CrosstermBackend};
use tui::{Frame, Terminal};

#[derive(Debug, Default)]
struct Ui {
    text: TextLineState,
}

impl Ui {
    fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        f.render_stateful_widget(TextLine, f.size(), &mut self.text);
        self.text.set_cursor(f, f.size());
    }
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut ui = Ui::default();
    loop {
        terminal.draw(|f| ui.draw(f))?;

        let event = crossterm::event::read()?;

        if let Event::Key(k) = event {
            if k.code == KeyCode::Esc {
                break;
            }
        }

        ui.text.process_input(event);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    crossterm::terminal::enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;

    // Defer error handling so the terminal always gets restored properly
    let result = run(&mut terminal);

    crossterm::terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    result?;

    Ok(())
}
