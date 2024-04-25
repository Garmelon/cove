// TODO Remove unnecessary Debug impls and compare compile times
// TODO Invoke external notification command?

mod euph;
mod export;
mod logger;
mod macros;
mod store;
mod ui;
mod util;
mod vault;
mod version;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use cove_config::doc::Document;
use cove_config::Config;
use directories::{BaseDirs, ProjectDirs};
use log::info;
use tokio::sync::mpsc;
use toss::Terminal;

use crate::logger::Logger;
use crate::ui::Ui;
use crate::vault::Vault;
use crate::version::{NAME, VERSION};

#[derive(Debug, clap::Parser)]
enum Command {
    /// Run the client interactively (default).
    Run,
    /// Export room logs as plain text files.
    Export(export::Args),
    /// Compact and clean up vault.
    Gc,
    /// Clear euphoria session cookies.
    ClearCookies {
        /// Clear cookies for a specific domain only.
        #[arg(long, short)]
        domain: Option<String>,
    },
    /// Print config documentation as markdown.
    HelpConfig,
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

fn config_path(args: &Args, dirs: &ProjectDirs) -> PathBuf {
    args.config
        .clone()
        .unwrap_or_else(|| dirs.config_dir().join("config.toml"))
}

fn data_dir(config: &Config, dirs: &ProjectDirs) -> PathBuf {
    config
        .data_dir
        .clone()
        .unwrap_or_else(|| dirs.data_dir().to_path_buf())
}

fn update_config_with_args(config: &mut Config, args: &Args) {
    if let Some(data_dir) = args.data_dir.clone() {
        // The data dir specified via args_data_dir is relative to the current
        // directory and needs no resolving.
        config.data_dir = Some(data_dir);
    } else if let Some(data_dir) = &config.data_dir {
        // Resolve the data dir specified in the config file relative to the
        // user's home directory, if possible.
        let base_dirs = BaseDirs::new().expect("failed to find home directory");
        config.data_dir = Some(base_dirs.home_dir().join(data_dir));
    }

    config.ephemeral |= args.ephemeral;
    config.measure_widths |= args.measure_widths;
    config.offline |= args.offline;
}

fn open_vault(config: &Config, dirs: &ProjectDirs) -> anyhow::Result<Vault> {
    let time_zone =
        util::load_time_zone(config.time_zone_ref()).context("failed to load time zone")?;
    let time_zone = Box::leak(Box::new(time_zone));

    let vault = if config.ephemeral {
        vault::launch_in_memory(time_zone)?
    } else {
        let data_dir = data_dir(config, dirs);
        eprintln!("Data dir:    {}", data_dir.to_string_lossy());
        vault::launch(&data_dir.join("vault.db"), time_zone)?
    };

    Ok(vault)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (logger, logger_guard, logger_rx) = Logger::init(args.verbose);
    let dirs = ProjectDirs::from("de", "plugh", "cove").expect("failed to find config directory");

    // Locate config
    let config_path = config_path(&args, &dirs);
    eprintln!("Config file: {}", config_path.to_string_lossy());

    // Load config
    let mut config = Config::load(&config_path)?;
    update_config_with_args(&mut config, &args);
    let config = Box::leak(Box::new(config));

    match args.command.unwrap_or_default() {
        Command::Run => run(logger, logger_rx, config, &dirs).await?,
        Command::Export(args) => export(config, &dirs, args).await?,
        Command::Gc => gc(config, &dirs).await?,
        Command::ClearCookies { domain } => clear_cookies(config, &dirs, domain).await?,
        Command::HelpConfig => help_config(),
    }

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
    dirs: &ProjectDirs,
) -> anyhow::Result<()> {
    info!("Welcome to {NAME} {VERSION}",);

    let vault = open_vault(config, dirs)?;

    let mut terminal = Terminal::new()?;
    terminal.set_measuring(config.measure_widths);
    Ui::run(config, &mut terminal, vault.clone(), logger, logger_rx).await?;
    drop(terminal);

    vault.close().await;
    Ok(())
}

async fn export(
    config: &'static Config,
    dirs: &ProjectDirs,
    args: export::Args,
) -> anyhow::Result<()> {
    let vault = open_vault(config, dirs)?;

    export::export(&vault.euph(), args).await?;

    vault.close().await;
    Ok(())
}

async fn gc(config: &'static Config, dirs: &ProjectDirs) -> anyhow::Result<()> {
    let vault = open_vault(config, dirs)?;

    eprintln!("Cleaning up and compacting vault");
    eprintln!("This may take a while...");
    vault.gc().await?;

    vault.close().await;
    Ok(())
}

async fn clear_cookies(
    config: &'static Config,
    dirs: &ProjectDirs,
    domain: Option<String>,
) -> anyhow::Result<()> {
    let vault = open_vault(config, dirs)?;

    eprintln!("Clearing cookies");
    vault.euph().clear_cookies(domain).await?;

    vault.close().await;
    Ok(())
}

fn help_config() {
    print!("{}", Config::doc().as_markdown());
}
