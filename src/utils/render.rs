//! How a payload becomes the text the model reads.
//!
//! Every tool returns a summary and fields. The summary is the answer in a sentence; the fields are what
//! a client reads programmatically, and the text shows them too, because the text is what the model
//! actually sees:
//!
//! - a scalar field is a bullet, `- key: value`;
//! - a list of scalars is one bullet, joined with commas;
//! - a list of rows (`items`, `files`, `commits`, `comments`) is a heading and one bullet per row, and a
//!   long or multi-line field of a row (a comment's body) is a block indented under its bullet;
//! - `text` is a block - a file, a log, a diff - fenced so it cannot be mistaken for prose;
//! - any other string with a line break, or too long for a bullet, is a block under its own name, so a
//!   commit message or an issue body keeps its lines;
//! - a nested object is left to the structured content.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::config::config::MAX_OUTPUT;
use crate::utils::text::{char_len, take_chars};

const CELL_MAX: usize = 200;

fn is_scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

/// A string that would not fit a bullet: it has a line break, or it is longer than a cell.
fn is_block_string(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\n') || char_len(s) > CELL_MAX,
        _ => false,
    }
}

/// A scalar as JavaScript's `String()` writes it.
pub fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js_number(n),
        Value::Null => "null".to_string(),
        Value::Array(items) => items
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// A number as JavaScript writes it: a whole number has no fraction.
pub fn js_number(n: &serde_json::Number) -> String {
    if n.is_i64() || n.is_u64() {
        return n.to_string();
    }
    match n.as_f64() {
        Some(f) if f.fract() == 0.0 && f.abs() < 1e21 => format!("{}", f as i128),
        Some(f) => format!("{f}"),
        None => n.to_string(),
    }
}

fn join_scalars(items: &[Value]) -> String {
    items.iter().map(js_string).collect::<Vec<_>>().join(", ")
}

static LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\n\s*").unwrap());

/// One value of a row, on one line.
fn cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Array(items) => {
            if items.iter().all(is_scalar) {
                join_scalars(items)
            } else {
                String::new()
            }
        }
        v if !is_scalar(v) => String::new(),
        v => {
            let line = LINE_BREAK.replace_all(&js_string(v), " ").into_owned();
            if char_len(&line) > CELL_MAX {
                format!("{}…", take_chars(&line, CELL_MAX))
            } else {
                line
            }
        }
    }
}

/// A row as one line: `key: value · key: value`, scalars only.
pub fn describe(row: &Value) -> String {
    if is_scalar(row) {
        return cell(row);
    }
    let Value::Object(map) = row else {
        return String::new();
    };
    map.iter()
        .filter(|(_, value)| !is_block_string(value))
        .map(|(key, value)| (key, cell(value)))
        .filter(|(_, text)| !text.is_empty())
        .map(|(key, text)| format!("{key}: {text}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// One row of a list: a bullet for its short fields, then each long field as an indented block.
fn render_row(row: &Value) -> String {
    let mut lines = vec![format!("- {}", describe(row))];
    if let Value::Object(map) = row {
        for (key, value) in map {
            if let (true, Value::String(text)) = (is_block_string(value), value) {
                let block = format!("{key}:\n{}", fence(text));
                lines.extend(block.split('\n').map(|line| format!("  {line}")));
            }
        }
    }
    lines.join("\n")
}

/// Fence a block with more backticks than any run inside it, so the content cannot close the fence.
pub fn fence(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let ticks = "`".repeat(3.max(longest + 1));
    let body = text.strip_suffix('\n').unwrap_or(text);
    format!("{ticks}text\n{body}\n{ticks}")
}

/// The text of a successful result.
pub fn render_payload(summary: &str, fields: &Map<String, Value>) -> String {
    let mut parts = vec![format!("**{summary}**")];
    let mut bullets: Vec<String> = Vec::new();
    let mut lists: Vec<(&str, &Vec<Value>)> = Vec::new();
    let mut blocks: Vec<(Option<&str>, &str)> = Vec::new();

    for (key, value) in fields {
        if value.is_null() {
            continue;
        }
        if let (true, Value::String(text)) = (key == "text", value) {
            if !text.is_empty() {
                blocks.push((None, text));
            }
        } else if let (true, Value::String(text)) = (is_block_string(value), value) {
            blocks.push((Some(key), text));
        } else if let Value::Array(items) = value {
            if items.is_empty() {
                continue;
            }
            if items.iter().all(is_scalar) {
                bullets.push(format!("- {key}: {}", join_scalars(items)));
            } else {
                lists.push((key, items));
            }
        } else if is_scalar(value) {
            bullets.push(format!("- {key}: {}", cell(value)));
        }
    }

    if !bullets.is_empty() {
        parts.push(String::new());
        parts.push(bullets.join("\n"));
    }
    for (key, rows) in lists {
        parts.push(String::new());
        parts.push(format!("**{key}**"));
        parts.push(rows.iter().map(render_row).collect::<Vec<_>>().join("\n"));
    }
    for (label, text) in blocks {
        parts.push(String::new());
        if let Some(label) = label {
            parts.push(format!("**{label}**"));
        }
        parts.push(fence(text));
    }

    let text = parts.join("\n");
    if char_len(&text) > MAX_OUTPUT {
        format!("{}\n... (truncated)", take_chars(&text, MAX_OUTPUT))
    } else {
        text
    }
}
