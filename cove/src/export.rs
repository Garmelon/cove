//! Export logs from the vault to plain text files.

mod json;
mod text;

use std::fs::File;
use std::io::{self, BufWriter, Write};

use crate::vault::{EuphRoomVault, EuphVault, RoomIdentifier};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Format {
    /// Human-readable tree-structured messages.
    Text,
    /// Array of message objects in the same format as the euphoria API uses.
    Json,
    /// Message objects in the same format as the euphoria API uses, one per
    /// line (https://jsonlines.org/).
    JsonLines,
}

impl Format {
    fn name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::JsonLines => "json lines",
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Json => "json",
            Self::JsonLines => "jsonl",
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct Args {
    rooms: Vec<String>,

    /// Export all rooms.
    #[arg(long, short)]
    all: bool,

    /// Domain to resolve the room names with.
    #[arg(long, short, default_value = "euphoria.leet.nu")]
    domain: String,

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
    /// If the value is a literal `-`, the export will be written to stdout. To
    /// write to a file named `-`, you can use `./-`.
    ///
    /// Must be a valid utf-8 encoded string.
    #[arg(long, short, default_value_t = Into::into("%r.%e"))]
    #[arg(verbatim_doc_comment)]
    out: String,
}

async fn export_room<W: Write>(
    vault: &EuphRoomVault,
    out: &mut W,
    format: Format,
) -> anyhow::Result<()> {
    match format {
        Format::Text => text::export(vault, out).await?,
        Format::Json => json::export(vault, out).await?,
        Format::JsonLines => json::export_lines(vault, out).await?,
    }
    Ok(())
}

pub async fn export(vault: &EuphVault, mut args: Args) -> anyhow::Result<()> {
    if args.out.ends_with('/') {
        args.out.push_str("%r.%e");
    }

    let rooms = if args.all {
        let mut rooms = vault
            .rooms()
            .await?
            .into_iter()
            .map(|id| id.name)
            .collect::<Vec<_>>();
        rooms.sort_unstable();
        rooms
    } else {
        let mut rooms = args.rooms.clone();
        rooms.dedup();
        rooms
    };

    if rooms.is_empty() {
        eprintln!("No rooms to export");
    }

    for room in rooms {
        if args.out == "-" {
            eprintln!("Exporting &{room} as {} to stdout", args.format.name());
            let vault = vault.room(RoomIdentifier::new(args.domain.clone(), room));
            let mut stdout = BufWriter::new(io::stdout());
            export_room(&vault, &mut stdout, args.format).await?;
            stdout.flush()?;
        } else {
            let out = format_out(&args.out, &room, args.format);
            eprintln!("Exporting &{room} as {} to {out}", args.format.name());
            let vault = vault.room(RoomIdentifier::new(args.domain.clone(), room));
            let mut file = BufWriter::new(File::create(out)?);
            export_room(&vault, &mut file, args.format).await?;
            file.flush()?;
        }
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
