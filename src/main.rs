#![deny(unsafe_code)]
// Rustc lint groups
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]
// Rustc lints
#![warn(noop_method_call)]
#![warn(single_use_lifetimes)]
#![warn(trivial_numeric_casts)]
#![warn(unused_crate_dependencies)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
// Clippy lints
#![warn(clippy::use_self)]

// TODO Enable warn(unreachable_pub)?
// TODO Clean up use and manipulation of toss Pos and Size

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
use toss::terminal::Terminal;
use ui::Ui;
use vault::Vault;

use crate::config::Config;
use crate::logger::Logger;

#[derive(Debug, clap::Subcommand)]
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
#[clap(version)]
struct Args {
    /// Path to the config file.
    #[clap(long, short)]
    config: Option<PathBuf>,

    /// Path to a directory for cove to store its data in.
    #[clap(long, short)]
    data_dir: Option<PathBuf>,

    /// If set, cove won't store data permanently.
    #[clap(long, short, action)]
    ephemeral: bool,

    /// If set, cove will ignore the autojoin config option.
    #[clap(long, short, action)]
    offline: bool,

    /// Measure the width of characters as displayed by the terminal emulator
    /// instead of guessing the width.
    #[clap(long, short, action)]
    measure_widths: bool,

    #[clap(subcommand)]
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
    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("unable to determine directories");

    let config_path = args
        .config
        .unwrap_or_else(|| dirs.config_dir().join("config.toml"));
    println!("Config file: {}", config_path.to_string_lossy());
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
        println!("Data dir:    {}", data_dir.to_string_lossy());
        vault::launch(&data_dir.join("vault.db"))?
    };

    match args.command.unwrap_or_default() {
        Command::Run => run(config, &vault, args.measure_widths).await?,
        Command::Export(args) => export::export(&vault, args).await?,
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

async fn run(config: &'static Config, vault: &Vault, measure_widths: bool) -> anyhow::Result<()> {
    let (logger, logger_rx) = Logger::init(log::Level::Debug);
    info!(
        "Welcome to {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let mut terminal = Terminal::new()?;
    terminal.set_measuring(measure_widths);
    Ui::run(config, &mut terminal, vault.clone(), logger, logger_rx).await?;
    drop(terminal); // So the vault can print again

    Ok(())
}
