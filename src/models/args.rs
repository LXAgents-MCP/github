//! Argument checks. Every tool validates its own arguments before it reads the token or sends anything,
//! so a bad call costs no request and the refusal names the argument.
//!
//! Arguments arrive as untyped JSON. A missing argument and an explicit `null` are the same thing here,
//! as `undefined` and `null` were.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::utils::errors::Refused;
use crate::utils::render::js_number;

/// A tool's arguments as the client sent them, before the tool has checked any of them.
pub type Args = Map<String, Value>;

static NULL: Value = Value::Null;

/// One argument, or `null` when it was not given.
pub fn arg<'a>(args: &'a Args, name: &str) -> &'a Value {
    args.get(name).unwrap_or(&NULL)
}

/// The name JavaScript's `typeof` gives a value, for a refusal.
pub fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "object",
    }
}

/// An optional string argument: trimmed, and `None` when absent or blank.
pub fn optional_text(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => {
            let trimmed = s.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        other => Err(Refused(format!(
            "`{field}` must be a string, got {}.",
            type_name(other)
        ))),
    }
}

/// A required string argument: trimmed, and never blank.
pub fn required_text(value: &Value, field: &str) -> Result<String, Refused> {
    optional_text(value, field)?.ok_or_else(|| {
        Refused(format!(
            "`{field}` is required and must be a non-empty string."
        ))
    })
}

/// Text kept exactly as given - a file's content, a comment body - and only checked to be a string.
pub fn raw_text(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        other => Err(Refused(format!(
            "`{field}` must be a string, got {}.",
            type_name(other)
        ))),
    }
}

/// Required text kept exactly as given; it may not be blank unless `allow_empty`.
pub fn required_raw_text(value: &Value, field: &str, allow_empty: bool) -> Result<String, Refused> {
    let Some(text) = raw_text(value, field)? else {
        return Err(Refused(format!("`{field}` is required.")));
    };
    if !allow_empty && text.trim().is_empty() {
        return Err(Refused(format!(
            "`{field}` is required and must not be blank."
        )));
    }
    Ok(text)
}

/// An optional boolean argument, with the value to use when it is absent.
pub fn optional_flag(value: &Value, field: &str, fallback: bool) -> Result<bool, Refused> {
    Ok(maybe_flag(value, field)?.unwrap_or(fallback))
}

/// An optional boolean with no default, so "not given" and "false" stay different.
pub fn maybe_flag(value: &Value, field: &str) -> Result<Option<bool>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        other => Err(Refused(format!(
            "`{field}` must be true or false, got {}.",
            type_name(other)
        ))),
    }
}

/// The range a whole number must fall in.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub min: i64,
    pub max: i64,
}

/// `Number.MAX_SAFE_INTEGER`.
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

impl Default for Range {
    fn default() -> Self {
        Range {
            min: 1,
            max: MAX_SAFE_INTEGER,
        }
    }
}

impl Range {
    pub fn min(min: i64) -> Self {
        Range {
            min,
            ..Default::default()
        }
    }
    pub fn max(max: i64) -> Self {
        Range {
            max,
            ..Default::default()
        }
    }
    pub fn between(min: i64, max: i64) -> Self {
        Range { min, max }
    }
}

/// An optional whole number inside a range.
pub fn optional_int(value: &Value, field: &str, range: Range) -> Result<Option<i64>, Refused> {
    let number = match value {
        Value::Null => return Ok(None),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else {
                n.as_f64()
                    .filter(|f| f.fract() == 0.0 && f.is_finite() && f.abs() < 9.3e18)
                    .map(|f| f as i64)
            }
        }
        _ => None,
    };
    let Some(number) = number else {
        return Err(Refused(format!(
            "`{field}` must be a whole number, got {value}."
        )));
    };
    if number < range.min || number > range.max {
        let got = match value {
            Value::Number(n) => js_number(n),
            _ => number.to_string(),
        };
        return Err(Refused(format!(
            "`{field}` must be between {} and {}, got {got}.",
            range.min, range.max
        )));
    }
    Ok(Some(number))
}

/// A required whole number inside a range.
pub fn required_int(value: &Value, field: &str, range: Range) -> Result<i64, Refused> {
    optional_int(value, field, range)?
        .ok_or_else(|| Refused(format!("`{field}` is required and must be a whole number.")))
}

static DATE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap());

/// Whether JavaScript's `Date.parse` reads `text` as a time, for the ISO 8601 forms it accepts: a date,
/// and a date with a time, seconds, a fraction and a zone, each optional after the date.
fn parses_as_date(text: &str) -> bool {
    use chrono::{DateTime, NaiveDate, NaiveDateTime};
    if NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() && text.len() == 10 {
        return true;
    }
    if DateTime::parse_from_rfc3339(text).is_ok() {
        return true;
    }
    let zoned = [
        "%Y-%m-%dT%H:%M%#z",
        "%Y-%m-%dT%H:%M:%S%#z",
        "%Y-%m-%dT%H:%M:%S%.f%#z",
    ];
    if zoned
        .iter()
        .any(|format| DateTime::parse_from_str(text, format).is_ok())
    {
        return true;
    }
    let naked = text.strip_suffix('Z').unwrap_or(text);
    [
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
    ]
    .iter()
    .any(|format| NaiveDateTime::parse_from_str(naked, format).is_ok())
}

/// An optional date or timestamp, in ISO 8601 form such as `2026-10-01` or `2026-10-01T12:00:00Z`.
pub fn optional_date(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    let Some(text) = optional_text(value, field)? else {
        return Ok(None);
    };
    if !DATE_START.is_match(&text) || !parses_as_date(&text) {
        return Err(Refused(format!(
            "`{field}` must be an ISO 8601 date such as 2026-10-01 or 2026-10-01T12:00:00Z, got '{text}'."
        )));
    }
    Ok(Some(text))
}

/// One of a fixed set of words, or `None` when absent.
pub fn one_of(value: &Value, field: &str, allowed: &[&str]) -> Result<Option<String>, Refused> {
    let Some(text) = optional_text(value, field)? else {
        return Ok(None);
    };
    if !allowed.contains(&text.as_str()) {
        return Err(Refused(format!(
            "`{field}` must be one of {}, got '{text}'.",
            allowed.join(", ")
        )));
    }
    Ok(Some(text))
}

/// One of a fixed set of words, with the value to use when it is absent.
pub fn one_of_or(
    value: &Value,
    field: &str,
    allowed: &[&str],
    fallback: &str,
) -> Result<String, Refused> {
    Ok(one_of(value, field, allowed)?.unwrap_or_else(|| fallback.to_string()))
}

/// A list of non-blank strings, trimmed and without duplicates, in the order first given.
pub fn text_list(
    value: &Value,
    field: &str,
    required: bool,
    max: usize,
) -> Result<Option<Vec<String>>, Refused> {
    let entries = match value {
        Value::Null => {
            if required {
                return Err(Refused(format!(
                    "`{field}` is required and must list at least one value."
                )));
            }
            return Ok(None);
        }
        Value::Array(entries) => entries,
        _ => return Err(Refused(format!("`{field}` must be a list of strings."))),
    };
    let mut seen: Vec<String> = Vec::new();
    for entry in entries {
        let Value::String(text) = entry else {
            return Err(Refused(format!("`{field}` must be a list of strings.")));
        };
        let text = text.trim();
        if !text.is_empty() && !seen.iter().any(|s| s == text) {
            seen.push(text.to_string());
        }
    }
    if seen.len() > max {
        return Err(Refused(format!(
            "`{field}` may hold at most {max} values, got {}.",
            seen.len()
        )));
    }
    if required && seen.is_empty() {
        return Err(Refused(format!(
            "`{field}` is required and must list at least one value."
        )));
    }
    Ok(Some(seen))
}
