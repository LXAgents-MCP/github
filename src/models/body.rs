//! A JSON object as GitHub returns it, read the way the TypeScript server read it.
//!
//! GitHub's answers are read defensively, field by field, with a fallback for every field that may be
//! missing, and a proxy or an outage can answer with something else entirely. Typing each object in full
//! would claim a shape this server never checks, so a body stays a `serde_json::Value` and these helpers
//! give the few JavaScript semantics the reads rely on: optional chaining, truthiness, `??`.
use serde_json::{Map, Value};

static NULL: Value = Value::Null;

/// `value?.[key]`: the field, or `null` when `value` is not an object or has no such field.
pub fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    match value {
        Value::Object(map) => map.get(key).unwrap_or(&NULL),
        _ => &NULL,
    }
}

/// `value?.a?.b`: a dotted path of `get`s.
pub fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    path.split('.')
        .fold(value, |current, key| get(current, key))
}

/// JavaScript truthiness: false for `null`, `false`, `0`, `NaN` and `""`; true for anything else.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// The value when it is a string.
pub fn as_str(value: &Value) -> Option<&str> {
    value.as_str()
}

/// The value when it is a non-empty string: `value ? { key: value } : {}` for a string field.
pub fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// The value when it is truthy, whatever its type.
pub fn present(value: &Value) -> Option<Value> {
    if truthy(value) {
        Some(value.clone())
    } else {
        None
    }
}

/// `value ?? fallback`.
pub fn or<'a>(value: &'a Value, fallback: &'a Value) -> &'a Value {
    if value.is_null() { fallback } else { value }
}

/// `value ?? 0` for a count.
pub fn count(value: &Value) -> Value {
    if value.is_null() {
        Value::from(0)
    } else {
        value.clone()
    }
}

/// `Boolean(value)`.
pub fn boolean(value: &Value) -> Value {
    Value::Bool(truthy(value))
}

/// `Array.isArray(value) ? value : []`.
pub fn list(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        _ => &[],
    }
}

/// `Number.isInteger(value)`.
pub fn is_int(value: &Value) -> bool {
    match value {
        Value::Number(n) => {
            n.is_i64()
                || n.is_u64()
                || n.as_f64()
                    .map(|f| f.fract() == 0.0 && f.is_finite())
                    .unwrap_or(false)
        }
        _ => false,
    }
}

/// `Number(text)` for a header value: blank is 0, anything that is not a number is NaN.
pub fn number_of(text: Option<&str>) -> f64 {
    let Some(text) = text else { return f64::NAN };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

/// An object built field by field, in order, as the TypeScript object literals were.
#[derive(Debug, Clone, Default)]
pub struct Fields(pub Map<String, Value>);

impl Fields {
    pub fn new() -> Self {
        Fields(Map::new())
    }

    /// `key: value`.
    pub fn set(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.to_string(), value.into());
        self
    }

    /// `...(value !== undefined ? { key: value } : {})`.
    pub fn opt<V: Into<Value>>(mut self, key: &str, value: Option<V>) -> Self {
        if let Some(value) = value {
            self.0.insert(key.to_string(), value.into());
        }
        self
    }

    /// `...other`.
    pub fn merge(mut self, other: Map<String, Value>) -> Self {
        for (key, value) in other {
            self.0.insert(key, value);
        }
        self
    }

    pub fn into_map(self) -> Map<String, Value> {
        self.0
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }
}

impl From<Fields> for Value {
    fn from(fields: Fields) -> Self {
        Value::Object(fields.0)
    }
}

/// `value ?? fallback`, as an owned value.
pub fn nn(value: &Value, fallback: impl Into<Value>) -> Value {
    if value.is_null() {
        fallback.into()
    } else {
        value.clone()
    }
}

/// `${value}` inside a template literal: the value as `String()` writes it.
pub fn s(value: &Value) -> String {
    crate::utils::render::js_string(value)
}

/// `count === 1 ? one : many`, for a count GitHub reported.
pub fn plural<'a>(count: &Value, one: &'a str, many: &'a str) -> &'a str {
    if count.as_f64() == Some(1.0) {
        one
    } else {
        many
    }
}

/// The changed files of a commit or a comparison, as the tools report them.
pub fn file_rows(files: &[Value], max: usize) -> Vec<Value> {
    files
        .iter()
        .take(max)
        .map(|file| {
            Fields::new()
                .set("filename", get(file, "filename").clone())
                .set("status", get(file, "status").clone())
                .set("additions", count(get(file, "additions")))
                .set("deletions", count(get(file, "deletions")))
                .into_value()
        })
        .collect()
}
