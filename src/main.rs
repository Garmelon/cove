#![warn(clippy::use_self)]

// TODO Clean up use and manipulation of toss Pos and Size

mod euph;
mod export;
mod logger;
mod macros;
mod replies;
mod store;
mod ui;
mod vault;

use std::path::PathBuf;

use clap::Parser;
use directories::ProjectDirs;
use log::info;
use toss::terminal::Terminal;
use ui::Ui;
use vault::Vault;

use crate::logger::Logger;

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Run the client interactively (default).
    Run,
    /// Export logs for a single room as a plain text file.
    Export { room: String, file: PathBuf },
    /// Compact and clean up vault.
    Gc,
}

impl Default for Command {
    fn default() -> Self {
        Self::Run
    }
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    command: Option<Command>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");
    println!("Data dir: {}", dirs.data_dir().to_string_lossy());

    let vault = vault::launch(&dirs.data_dir().join("vault.db"))?;

    match args.command.unwrap_or_default() {
        Command::Run => run(&vault).await?,
        Command::Export { room, file } => export::export(&vault, room, &file).await?,
        Command::Gc => {
            println!("Cleaning up and compacting vault");
            println!("This may take a while...");
            vault.gc().await;
        }
    }

    vault.close().await;

    println!("Goodbye!");
    Ok(())
}

async fn run(vault: &Vault) -> anyhow::Result<()> {
    let (logger, logger_rx) = Logger::init(log::Level::Debug);
    info!(
        "Welcome to {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let mut terminal = Terminal::new()?;
    // terminal.set_measuring(true);
    Ui::run(&mut terminal, vault.clone(), logger, logger_rx).await?;
    drop(terminal); // So the vault can print again

    Ok(())
}
