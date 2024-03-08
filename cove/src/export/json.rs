use std::io::Write;

use crate::vault::EuphRoomVault;

const CHUNK_SIZE: usize = 10000;

pub async fn export<W: Write>(vault: &EuphRoomVault, file: &mut W) -> anyhow::Result<()> {
    write!(file, "[")?;

    let mut total = 0;
    let mut last_msg_id = None;
    loop {
        let messages = vault.chunk_after(last_msg_id, CHUNK_SIZE).await?;
        last_msg_id = Some(match messages.last() {
            Some(last_msg) => last_msg.id,
            None => break, // No more messages, export finished
        });

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
            eprintln!("  {total} messages");
        }
    }

    write!(file, "\n]")?;

    eprintln!("  {total} messages in total");
    Ok(())
}

pub async fn export_lines<W: Write>(vault: &EuphRoomVault, file: &mut W) -> anyhow::Result<()> {
    let mut total = 0;
    let mut last_msg_id = None;
    loop {
        let messages = vault.chunk_after(last_msg_id, CHUNK_SIZE).await?;
        last_msg_id = Some(match messages.last() {
            Some(last_msg) => last_msg.id,
            None => break, // No more messages, export finished
        });

        for message in messages {
            serde_json::to_writer(&mut *file, &message)?; // Fancy reborrow! :D
            writeln!(file)?;
            total += 1;
        }

        if total % 100000 == 0 {
            eprintln!("  {total} messages");
        }
    }

    eprintln!("  {total} messages in total");
    Ok(())
}
