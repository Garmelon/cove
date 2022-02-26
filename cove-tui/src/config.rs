use std::time::{Duration, Instant};

use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[clap(long, default_value_t = String::from("wss://plugh.de/cove/"))]
    cove_url: String,
}

pub struct Config {
    pub cove_url: String,
    pub cove_identity: String,
    pub timeout: Duration,
}

impl Config {
    pub fn load() -> Self {
        let args = Args::parse();
        Self {
            cove_url: args.cove_url,
            // TODO Load identity from file oslt
            cove_identity: format!("{:?}", Instant::now()),
            timeout: Duration::from_secs(10),
        }
    }
}
