use std::io::Write;

use euphoxide::api::MessageId;
use unicode_width::UnicodeWidthStr;

use crate::{euph::SmallMessage, store::Tree, vault::EuphRoomVault};

const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const TIME_EMPTY: &str = "                   ";

pub async fn export<W: Write>(vault: &EuphRoomVault, out: &mut W) -> anyhow::Result<()> {
    let mut exported_trees = 0;
    let mut exported_msgs = 0;
    let mut root_id = vault.first_root_id().await?;
    while let Some(some_root_id) = root_id {
        let tree = vault.tree(some_root_id).await?;
        write_tree(out, &tree, some_root_id, 0)?;
        root_id = vault.next_root_id(some_root_id).await?;

        exported_trees += 1;
        exported_msgs += tree.len();

        if exported_trees % 10000 == 0 {
            eprintln!("  {exported_trees} trees, {exported_msgs} messages")
        }
    }
    eprintln!("  {exported_trees} trees, {exported_msgs} messages in total");

    Ok(())
}

fn write_tree<W: Write>(
    out: &mut W,
    tree: &Tree<SmallMessage>,
    id: MessageId,
    indent: usize,
) -> anyhow::Result<()> {
    let indent_string = "| ".repeat(indent);

    if let Some(msg) = tree.msg(&id) {
        write_msg(out, &indent_string, msg)?;
    } else {
        write_placeholder(out, &indent_string)?;
    }

    if let Some(children) = tree.children(&id) {
        for child in children {
            write_tree(out, tree, *child, indent + 1)?;
        }
    }

    Ok(())
}

fn write_msg<W: Write>(
    file: &mut W,
    indent_string: &str,
    msg: &SmallMessage,
) -> anyhow::Result<()> {
    let nick = &msg.nick;
    let nick_empty = " ".repeat(nick.width());

    for (i, line) in msg.content.lines().enumerate() {
        if i == 0 {
            let time = msg.time.as_timestamp().strftime(TIME_FORMAT);
            writeln!(file, "{time} {indent_string}[{nick}] {line}")?;
        } else {
            writeln!(file, "{TIME_EMPTY} {indent_string}| {nick_empty} {line}")?;
        }
    }

    Ok(())
}

fn write_placeholder<W: Write>(file: &mut W, indent_string: &str) -> anyhow::Result<()> {
    writeln!(file, "{TIME_EMPTY} {indent_string}[...]")?;
    Ok(())
}
