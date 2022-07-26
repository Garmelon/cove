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
use cookie::CookieJar;
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
    /// Clear euphoria session cookies.
    ClearCookies,
}

impl Default for Command {
    fn default() -> Self {
        Self::Run
    }
}

#[derive(Debug, clap::Parser)]
struct Args {
    /// Path to a directory for cove to store its data in.
    #[clap(long, short)]
    data_dir: Option<PathBuf>,
    #[clap(subcommand)]
    command: Option<Command>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let data_dir = if let Some(data_dir) = args.data_dir {
        data_dir
    } else {
        let dirs =
            ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");
        dirs.data_dir().to_path_buf()
    };
    println!("Data dir: {}", data_dir.to_string_lossy());

    let vault = vault::launch(&data_dir.join("vault.db"))?;

    match args.command.unwrap_or_default() {
        Command::Run => run(&vault).await?,
        Command::Export { room, file } => export::export(&vault, room, &file).await?,
        Command::Gc => {
            println!("Cleaning up and compacting vault");
            println!("This may take a while...");
            vault.gc().await;
        }
        Command::ClearCookies => {
            println!("Clearing cookies");
            vault.set_euph_cookies(CookieJar::new());
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
