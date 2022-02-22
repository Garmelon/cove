mod config;
mod never;
mod replies;
mod room;
mod textline;
mod ui;

use std::io::{self, Stdout};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use tui::backend::CrosstermBackend;
use tui::Terminal;
use ui::Ui;

async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut ui = Ui::default();
    loop {
        ui.render_to_terminal(terminal).await?;

        let event = crossterm::event::read()?;

        if let Event::Key(k) = event {
            if k.code == KeyCode::Esc {
                break;
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
