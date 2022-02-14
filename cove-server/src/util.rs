use std::cmp;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("executed after 1970")
        .as_millis()
}

pub fn timestamp_after(previous: u128) -> u128 {
    cmp::max(timestamp(), previous + 1)
}

pub fn check_room(room: &str) -> Option<String> {
    if room.is_empty() {
        return Some("is empty".to_string());
    }
    if !room.is_ascii() {
        return Some("contains non-ascii characters".to_string());
    }
    if room.len() > 1024 {
        return Some("contains more than 1024 characters".to_string());
    }
    if !room
        .chars()
        .all(|c| c == '-' || c == '.' || ('a'..='z').contains(&c))
    {
        return Some("must only contain a-z, '-' and '_'".to_string());
    }
    None
}

pub fn check_nick(nick: &str) -> Option<String> {
    if nick.is_empty() {
        return Some("is empty".to_string());
    }
    if nick.trim().is_empty() {
        return Some("contains only whitespace".to_string());
    }
    let nick = nick.trim();
    if nick.chars().count() > 1024 {
        return Some("contains more than 1024 characters".to_string());
    }
    None
}

pub fn check_identity(identity: &str) -> Option<String> {
    if identity.chars().count() > 32 * 1024 {
        return Some("contains more than 32768 characters".to_string());
    }
    None
}

pub fn check_content(content: &str) -> Option<String> {
    if content.chars().count() > 128 * 1024 {
        return Some("contains more than 131072 characters".to_string());
    }
    None
}
