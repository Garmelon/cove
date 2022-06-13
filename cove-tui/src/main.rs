#![warn(clippy::use_self)]

mod chat;
mod store;
mod ui;

use toss::terminal::Terminal;
use ui::Ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut terminal = Terminal::new()?;
    // terminal.set_measuring(true);
    Ui::run(&mut terminal).await?;
    Ok(())
}
