#![forbid(unsafe_code)]
// Rustc lint groups
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]
#![warn(unused)]
// Rustc lints
#![warn(noop_method_call)]
#![warn(single_use_lifetimes)]
// Clippy lints
#![warn(clippy::use_self)]

// TODO Enable warn(unreachable_pub)?
// TODO Remove unnecessary Debug impls and compare compile times
// TODO Time zones other than UTC
// TODO Fix password room auth

mod config;
mod euph;
mod export;
mod logger;
mod macros;
mod store;
mod ui;
mod vault;

use std::path::PathBuf;

use clap::Parser;
use cookie::CookieJar;
use directories::{BaseDirs, ProjectDirs};
use log::info;
use tokio::sync::mpsc;
use toss::terminal::Terminal;

use crate::config::Config;
use crate::logger::Logger;
use crate::ui::Ui;
use crate::vault::Vault;

#[derive(Debug, clap::Parser)]
enum Command {
    /// Run the client interactively (default).
    Run,
    /// Export room logs as plain text files.
    Export(export::Args),
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
#[command(version)]
struct Args {
    /// Show more detailed log messages.
    #[arg(long, short)]
    verbose: bool,

    /// Path to the config file.
    ///
    /// Relative paths are interpreted relative to the current directory.
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Path to a directory for cove to store its data in.
    ///
    /// Relative paths are interpreted relative to the current directory.
    #[arg(long, short)]
    data_dir: Option<PathBuf>,

    /// If set, cove won't store data permanently.
    #[arg(long, short)]
    ephemeral: bool,

    /// If set, cove will ignore the autojoin config option.
    #[arg(long, short)]
    offline: bool,

    /// Measure the width of characters as displayed by the terminal emulator
    /// instead of guessing the width.
    #[arg(long, short)]
    measure_widths: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

fn set_data_dir(config: &mut Config, args_data_dir: Option<PathBuf>) {
    if let Some(data_dir) = args_data_dir {
        // The data dir specified via args_data_dir is relative to the current
        // directory and needs no resolving.
        config.data_dir = Some(data_dir);
    } else if let Some(data_dir) = &config.data_dir {
        // Resolve the data dir specified in the config file relative to the
        // user's home directory, if possible.
        if let Some(base_dirs) = BaseDirs::new() {
            config.data_dir = Some(base_dirs.home_dir().join(data_dir));
        }
    }
}

fn set_ephemeral(config: &mut Config, args_ephemeral: bool) {
    if args_ephemeral {
        config.ephemeral = true;
    }
}

fn set_offline(config: &mut Config, args_offline: bool) {
    if args_offline {
        config.offline = true;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (logger, logger_guard, logger_rx) = Logger::init(args.verbose);
    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");

    let config_path = args
        .config
        .unwrap_or_else(|| dirs.config_dir().join("config.toml"));
    eprintln!("Config file: {}", config_path.to_string_lossy());
    let mut config = Config::load(&config_path);
    set_data_dir(&mut config, args.data_dir);
    set_ephemeral(&mut config, args.ephemeral);
    set_offline(&mut config, args.offline);
    let config = Box::leak(Box::new(config));

    let vault = if config.ephemeral {
        vault::launch_in_memory()?
    } else {
        let data_dir = config
            .data_dir
            .clone()
            .unwrap_or_else(|| dirs.data_dir().to_path_buf());
        eprintln!("Data dir:    {}", data_dir.to_string_lossy());
        vault::launch(&data_dir.join("vault.db"))?
    };

    match args.command.unwrap_or_default() {
        Command::Run => run(logger, logger_rx, config, &vault, args.measure_widths).await?,
        Command::Export(args) => export::export(&vault.euph(), args).await?,
        Command::Gc => {
            eprintln!("Cleaning up and compacting vault");
            eprintln!("This may take a while...");
            vault.gc().await?;
        }
        Command::ClearCookies => {
            eprintln!("Clearing cookies");
            vault.euph().set_cookies(CookieJar::new()).await?;
        }
    }

    vault.close().await;

    // Print all logged errors. This should always happen, even if cove panics,
    // because the errors may be key in diagnosing what happened. Because of
    // this, it is not implemented via a normal function call.
    drop(logger_guard);

    eprintln!("Goodbye!");
    Ok(())
}

async fn run(
    logger: Logger,
    logger_rx: mpsc::UnboundedReceiver<()>,
    config: &'static Config,
    vault: &Vault,
    measure_widths: bool,
) -> anyhow::Result<()> {
    info!(
        "Welcome to {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let mut terminal = Terminal::new()?;
    terminal.set_measuring(measure_widths);
    Ui::run(config, &mut terminal, vault.clone(), logger, logger_rx).await?;
    drop(terminal); // So other things can print again

    Ok(())
}
