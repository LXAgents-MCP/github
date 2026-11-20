//! The one place a status GitHub answered with becomes a message that names the next step.
use std::collections::BTreeMap;
use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::Value;

use crate::config::config::TOKEN_ENV;
use crate::models::body::{get, number_of, truthy};
use crate::utils::errors::Refused;
use crate::utils::redact::redact;
use crate::utils::render::js_string;

/// A tool's own wording for the statuses it knows: given the status and GitHub's detail suffix, a
/// message, or `None` to fall through to the general wording.
pub type Explain<'a> = &'a (dyn Fn(u16, &str) -> Option<String> + Send + Sync);

/// What GitHub said went wrong, as a short suffix: ": message (detail; detail)".
pub fn api_detail(body: &Value) -> String {
    if let Value::String(text) = body {
        return if text.is_empty() {
            String::new()
        } else {
            format!(": {text}")
        };
    }
    let Some(message) = get(body, "message").as_str() else {
        return String::new();
    };
    let extra: Vec<String> = match get(body, "errors") {
        Value::Array(errors) => errors
            .iter()
            .map(|e| match e {
                Value::String(_) => e.clone(),
                _ => {
                    let message = get(e, "message");
                    if message.is_null() {
                        get(e, "code").clone()
                    } else {
                        message.clone()
                    }
                }
            })
            .filter(truthy)
            .map(|v| js_string(&v))
            .collect(),
        _ => Vec::new(),
    };
    if extra.is_empty() {
        format!(": {message}")
    } else {
        format!(": {message} ({})", extra.join("; "))
    }
}

static RATE_LIMIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)rate limit").unwrap());

/// Is this answer GitHub saying "too many requests" rather than "you may not"?
fn is_rate_limited(status: u16, headers: &BTreeMap<String, String>, body: &Value) -> bool {
    status == 429
        || headers.get("x-ratelimit-remaining").map(String::as_str) == Some("0")
        || headers.contains_key("retry-after")
        || RATE_LIMIT.is_match(&api_detail(body))
}

/// When the limit lifts, in words the model can act on.
fn when_to_retry(headers: &BTreeMap<String, String>) -> String {
    let seconds = number_of(headers.get("retry-after").map(String::as_str));
    if seconds.is_finite() && seconds > 0.0 {
        return format!("Wait {} seconds and try again.", seconds.ceil() as i64);
    }
    let reset = number_of(headers.get("x-ratelimit-reset").map(String::as_str));
    if reset.is_finite() && reset > 0.0 {
        let now = Utc::now().timestamp_millis() as f64;
        let minutes = ((reset * 1000.0 - now) / 60_000.0).ceil().max(1.0) as i64;
        let at = DateTime::<Utc>::from_timestamp_millis((reset * 1000.0) as i64)
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_default();
        let plural = if minutes == 1 { "" } else { "s" };
        return format!("The limit resets at {at}, in about {minutes} minute{plural}.");
    }
    "Wait a minute and try again.".to_string()
}

/// A refusal that says what to do next, for a status GitHub answered with.
///
/// `explain` lets a tool word the statuses it knows better than the general case. It receives the status
/// and GitHub's own detail suffix, and returns a message, or `None` to fall through to the general
/// wording below.
pub fn failure(
    status: u16,
    body: &Value,
    headers: &BTreeMap<String, String>,
    what: &str,
    explain: Option<Explain<'_>>,
    token: &str,
) -> Refused {
    let detail = api_detail(body);
    let message = explain.and_then(|explain| explain(status, &detail)).unwrap_or_else(|| {
        if status == 401 {
            format!("GitHub rejected the token (401). Check that {TOKEN_ENV} holds a valid token that has not expired.")
        } else if (status == 403 || status == 429) && is_rate_limited(status, headers, body) {
            format!("GitHub is rate limiting this token ({status}){detail}. {}", when_to_retry(headers))
        } else if status == 403 {
            let accepts = headers
                .get("x-accepted-github-permissions")
                .filter(|accepts| !accepts.is_empty())
                .map(|accepts| format!(" (GitHub says this endpoint accepts: {accepts})"))
                .unwrap_or_default();
            format!(
                "GitHub refused to {what} (403){detail}. The token lacks permission for this: a classic token needs the matching scope (repo, workflow or delete_repo), a fine-grained token needs the matching repository permission{accepts}."
            )
        } else if status == 404 {
            format!(
                "GitHub could not find what it needs to {what} (404){detail}. Check the spelling. GitHub also answers 404 for a private repository the token cannot see."
            )
        } else if status == 409 {
            format!("GitHub could not {what} (409){detail}. Something changed or conflicts; read it again before retrying.")
        } else if status == 422 {
            format!("GitHub could not {what} (422){detail}.")
        } else if status >= 500 {
            format!("GitHub failed to {what} ({status}){detail}. Try again shortly.")
        } else {
            format!("GitHub answered {status}{detail} when asked to {what}.")
        }
    });
    Refused(redact(&message, token))
}
