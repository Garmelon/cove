use std::time::Duration;

use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[clap(long, default_value_t = String::from("wss://plugh.de/cove/"))]
    cove_url: String,
}

pub struct Config {
    pub cove_url: String,
    pub timeout: Duration,
}

impl Config {
    pub fn load() -> Self {
        let args = Args::parse();
        Self {
            cove_url: args.cove_url,
            timeout: Duration::from_secs(10),
        }
    }
}
