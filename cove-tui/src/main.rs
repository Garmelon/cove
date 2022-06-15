#![warn(clippy::use_self)]

mod chat;
mod store;
mod ui;
mod vault;

use directories::ProjectDirs;
use toss::terminal::Terminal;
use ui::Ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");
    println!("Data dir: {}", dirs.data_dir().to_string_lossy());

    let vault = vault::launch(&dirs.data_dir().join("vault.db"))?;
    let mut terminal = Terminal::new()?;
    // terminal.set_measuring(true);

    Ui::run(&mut terminal).await?;
    vault.close().await;
    Ok(())
}
