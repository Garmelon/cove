use std::fs::File;
use std::io::{BufWriter, Write};

use crate::vault::EuphRoomVault;

const CHUNK_SIZE: usize = 10000;

pub async fn export_to_file(
    vault: &EuphRoomVault,
    file: &mut BufWriter<File>,
) -> anyhow::Result<()> {
    write!(file, "[")?;

    let mut total = 0;
    let mut offset = 0;
    loop {
        let messages = vault.chunk_at_offset(CHUNK_SIZE, offset).await;
        offset += messages.len();

        if messages.is_empty() {
            break;
        }

        for message in messages {
            if total == 0 {
                writeln!(file)?;
            } else {
                writeln!(file, ",")?;
            }
            serde_json::to_writer(&mut *file, &message)?; // Fancy reborrow! :D
            total += 1;
        }

        if total % 100000 == 0 {
            println!("  {total} messages");
        }
    }

    write!(file, "\n]")?;

    println!("  {total} messages in total");

    Ok(())
}
