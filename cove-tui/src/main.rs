#![warn(clippy::use_self)]

mod ui;

use toss::terminal::Terminal;
use ui::Ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut terminal = Terminal::new()?;
    Ui::run(&mut terminal).await?;
    Ok(())
}
