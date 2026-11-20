//! Small text helpers shared by the tools. Pure functions, no I/O.
//!
//! Lengths and cuts count Unicode scalar values (`char`s). JavaScript counted UTF-16 code units; the two
//! agree everywhere in the Basic Multilingual Plane, and a cut here can never split a character.
use serde_json::Value;
use std::sync::LazyLock;

use regex::Regex;

/// The abbreviated form of a commit sha.
pub fn short(sha: &Value) -> String {
    match sha {
        Value::String(s) => take_chars(s, 7),
        _ => String::new(),
    }
}

/// The first line of a message, which is all a listing needs.
pub fn first_line(message: &Value) -> String {
    match message {
        Value::String(s) => match s.find('\n') {
            Some(end) => s[..end].strip_suffix('\r').unwrap_or(&s[..end]).to_string(),
            None => s.clone(),
        },
        _ => String::new(),
    }
}

/// Decode bytes as text: UTF-8 when they are valid UTF-8, Windows-1252 when they are not.
///
/// The byte order mark is kept, so a caller that cares can strip it explicitly.
pub fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
            text.into_owned()
        }
    }
}

/// The number of characters in `text`.
pub fn char_len(text: &str) -> usize {
    text.chars().count()
}

/// The first `max` characters of `text`.
pub fn take_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => text[..end].to_string(),
        None => text.to_string(),
    }
}

/// The last `max` characters of `text`.
pub fn last_chars(text: &str, max: usize) -> String {
    let len = char_len(text);
    if len <= max {
        return text.to_string();
    }
    match text.char_indices().nth(len - max) {
        Some((start, _)) => text[start..].to_string(),
        None => String::new(),
    }
}

/// Cut text to a number of characters, never leaving half a character at the cut.
///
/// Returns the text and whether it was cut.
pub fn cut_text(text: &str, max: usize) -> (String, bool) {
    if char_len(text) <= max {
        (text.to_string(), false)
    } else {
        (take_chars(text, max), true)
    }
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").unwrap());

/// A job's log as text to read: no byte order mark, one kind of line break, and none of the colour codes
/// that tools write for a terminal, which only add noise.
pub fn clean_log(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    ANSI.replace_all(&text, "").into_owned()
}

/// Does this look like binary data? A NUL byte early on is the usual tell.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|&b| b == 0)
}

/// Cut text to a length, keeping the start. Returns the text and whether it was cut.
pub fn head(text: &str, max: usize) -> (String, bool) {
    if char_len(text) > max {
        (format!("{}\n... (truncated)", take_chars(text, max)), true)
    } else {
        (text.to_string(), false)
    }
}

/// Cut text to a length, keeping the end - where a log says why it failed. Returns the text and whether
/// it was cut.
pub fn tail(text: &str, max: usize) -> (String, bool) {
    if char_len(text) > max {
        (format!("(truncated) ...\n{}", last_chars(text, max)), true)
    } else {
        (text.to_string(), false)
    }
}
