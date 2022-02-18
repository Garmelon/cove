mod replies;
mod room;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use tokio::time;
use tui::backend::CrosstermBackend;
use tui::widgets::{Block, Borders};
use tui::Terminal;

async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    terminal.draw(|f| {
        let block = Block::default().title("Block").borders(Borders::ALL);
        f.render_widget(block, f.size());
    })?;
    time::sleep(Duration::from_secs(1)).await;
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
