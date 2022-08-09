//! Export logs from the vault to plain text files.

mod text;

use std::fs::File;
use std::io::{BufWriter, Write};

use crate::vault::Vault;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Format {
    /// Human-readable tree-structured messages.
    Text,
}

impl Format {
    fn name(&self) -> &'static str {
        match self {
            Self::Text => "text",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Text => "txt",
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct Args {
    rooms: Vec<String>,

    /// Export all rooms.
    #[clap(long, short)]
    all: bool,

    /// Format of the output file.
    #[clap(long, short, value_enum, default_value_t = Format::Text)]
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
    #[clap(long, short, default_value_t = Into::into("%r.%e"))]
    #[clap(verbatim_doc_comment)]
    out: String,
}

pub async fn export(vault: &Vault, mut args: Args) -> anyhow::Result<()> {
    if args.out.ends_with('/') {
        args.out.push_str("%r.%e");
    }

    let rooms = if args.all {
        let mut rooms = vault.euph_rooms().await;
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

        let mut file = BufWriter::new(File::create(out)?);
        match args.format {
            Format::Text => text::export_to_file(vault, room, &mut file).await?,
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
