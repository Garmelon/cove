//! Export logs from the vault to plain text files.

mod json;
mod text;

use std::fs::File;
use std::io::{BufWriter, Write};

use crate::vault::EuphVault;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Format {
    /// Human-readable tree-structured messages.
    Text,
    /// Array of message objects in the same format as the euphoria API uses.
    Json,
    /// Message objects in the same format as the euphoria API uses, one per line.
    JsonStream,
}

impl Format {
    fn name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::JsonStream => "json stream",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Json | Self::JsonStream => "json",
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct Args {
    rooms: Vec<String>,

    /// Export all rooms.
    #[arg(long, short)]
    all: bool,

    /// Format of the output file.
    #[arg(long, short, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Location of the output file
    ///
    /// May include the following placeholders:
    /// `%r` - room name
    /// `%e` - format extension
    /// A literal `%` can be written as `%%`.
    ///
    /// If the value ends with a `/`, it is assumed to point to a directory and
    /// `%r.%e` will be appended.
    ///
    /// Must be a valid utf-8 encoded string.
    #[arg(long, short, default_value_t = Into::into("%r.%e"))]
    #[arg(verbatim_doc_comment)]
    out: String,
}

pub async fn export(vault: &EuphVault, mut args: Args) -> anyhow::Result<()> {
    if args.out.ends_with('/') {
        args.out.push_str("%r.%e");
    }

    let rooms = if args.all {
        let mut rooms = vault.rooms().await;
        rooms.sort_unstable();
        rooms
    } else {
        let mut rooms = args.rooms.clone();
        rooms.dedup();
        rooms
    };

    if rooms.is_empty() {
        println!("No rooms to export");
    }

    for room in rooms {
        let out = format_out(&args.out, &room, args.format);
        println!("Exporting &{room} as {} to {out}", args.format.name());

        let vault = vault.room(room);
        let mut file = BufWriter::new(File::create(out)?);
        match args.format {
            Format::Text => text::export_to_file(&vault, &mut file).await?,
            Format::Json => json::export_to_file(&vault, &mut file).await?,
            Format::JsonStream => json::export_stream_to_file(&vault, &mut file).await?,
        }
        file.flush()?;
    }

    Ok(())
}

fn format_out(out: &str, room: &str, format: Format) -> String {
    let mut result = String::new();

    let mut special = false;
    for char in out.chars() {
        if special {
            match char {
                'r' => result.push_str(room),
                'e' => result.push_str(format.extension()),
                '%' => result.push('%'),
                _ => {
                    result.push('%');
                    result.push(char);
                }
            }
            special = false;
        } else if char == '%' {
            special = true;
        } else {
            result.push(char);
        }
    }

    result
}
