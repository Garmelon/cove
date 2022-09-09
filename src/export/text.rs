use std::fs::File;
use std::io::{BufWriter, Write};

use euphoxide::api::Snowflake;
use time::format_description::FormatItem;
use time::macros::format_description;
use unicode_width::UnicodeWidthStr;

use crate::euph::SmallMessage;
use crate::store::Tree;
use crate::vault::EuphRoomVault;

const TIME_FORMAT: &[FormatItem<'_>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
const TIME_EMPTY: &str = "                   ";

pub async fn export_to_file(
    vault: &EuphRoomVault,
    file: &mut BufWriter<File>,
) -> anyhow::Result<()> {
    let mut exported_trees = 0;
    let mut exported_msgs = 0;
    let mut root_id = vault.first_root_id().await;
    while let Some(some_root_id) = root_id {
        let tree = vault.tree(some_root_id).await;
        write_tree(file, &tree, some_root_id, 0)?;
        root_id = vault.next_root_id(some_root_id).await;

        exported_trees += 1;
        exported_msgs += tree.len();

        if exported_trees % 10000 == 0 {
            println!("  {exported_trees} trees, {exported_msgs} messages")
        }
    }
    println!("  {exported_trees} trees, {exported_msgs} messages in total");

    Ok(())
}

fn write_tree(
    file: &mut BufWriter<File>,
    tree: &Tree<SmallMessage>,
    id: Snowflake,
    indent: usize,
) -> anyhow::Result<()> {
    let indent_string = "| ".repeat(indent);

    if let Some(msg) = tree.msg(&id) {
        write_msg(file, &indent_string, msg)?;
    } else {
        write_placeholder(file, &indent_string)?;
    }

    if let Some(children) = tree.children(&id) {
        for child in children {
            write_tree(file, tree, *child, indent + 1)?;
        }
    }

    Ok(())
}

fn write_msg(
    file: &mut BufWriter<File>,
    indent_string: &str,
    msg: &SmallMessage,
) -> anyhow::Result<()> {
    let nick = &msg.nick;
    let nick_empty = " ".repeat(nick.width());

    for (i, line) in msg.content.lines().enumerate() {
        if i == 0 {
            let time = msg
                .time
                .0
                .format(TIME_FORMAT)
                .expect("time can be formatted");
            writeln!(file, "{time} {indent_string}[{nick}] {line}")?;
        } else {
            writeln!(file, "{TIME_EMPTY} {indent_string}| {nick_empty} {line}")?;
        }
    }

    Ok(())
}

fn write_placeholder(file: &mut BufWriter<File>, indent_string: &str) -> anyhow::Result<()> {
    writeln!(file, "{TIME_EMPTY} {indent_string}[...]")?;
    Ok(())
}
