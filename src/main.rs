#![warn(clippy::use_self)]

// TODO Clean up use and manipulation of toss Pos and Size

mod chat;
mod euph;
mod logger;
mod replies;
mod store;
mod ui;
mod vault;

use directories::ProjectDirs;
use log::info;
use toss::terminal::Terminal;
use ui::Ui;

use crate::logger::Logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (logger, logger_rx) = Logger::init(log::Level::Debug);
    info!(
        "Welcome to {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");
    println!("Data dir: {}", dirs.data_dir().to_string_lossy());

    let vault = vault::launch(&dirs.data_dir().join("vault.db"))?;

    let mut terminal = Terminal::new()?;
    // terminal.set_measuring(true);
    Ui::run(&mut terminal, vault.clone(), logger, logger_rx).await?;
    drop(terminal); // So the vault can print again

    vault.close().await;

    println!("Goodbye!");
    Ok(())
}
