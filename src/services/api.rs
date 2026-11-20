//! The GitHub client: the one place a GitHub URL is built and a call is made.
//!
//! Every tool reaches GitHub through `github()`, and nothing else builds a URL, so nothing else can let an
//! argument steer a call at a different endpoint. The parts of a path come only from
//! `src/models/address.rs`, which validates and percent-encodes them; the request itself is
//! `src/services/client.rs`, and a refusal's wording is `src/services/failure.rs`.
//!
//! Nothing here writes to stdout: on stdio, stdout is the protocol channel.
use serde_json::Value;

use super::auth::require_token;
use super::client::{ApiAnswer, CallOptions, call_api};
use super::context::Ctx;
use super::failure::{Explain, failure};
use crate::config::config::{API_BASE, API_VERSION};
use crate::config::version::VERSION;
use crate::utils::errors::Refused;
use crate::utils::render::js_string;

pub const JSON_ACCEPT: &str = "application/vnd.github+json";

/// How a tool asks for one GitHub call.
#[derive(Default)]
pub struct GitHubOptions<'a> {
    /// Query parameters, in order; a `null` or empty value is left out.
    pub query: Vec<(&'a str, Value)>,
    pub json: Option<Value>,
    pub accept: Option<&'a str>,
    /// Statuses that are an answer and not a failure for this call.
    pub allow: &'a [u16],
    /// Completes "... to <what>" in a refusal.
    pub what: Option<String>,
    pub explain: Option<Explain<'a>>,
    pub raw: bool,
}

impl<'a> GitHubOptions<'a> {
    pub fn what(what: impl Into<String>) -> Self {
        GitHubOptions {
            what: Some(what.into()),
            ..Default::default()
        }
    }

    pub fn query(mut self, key: &'a str, value: impl Into<Value>) -> Self {
        self.query.push((key, value.into()));
        self
    }

    pub fn json(mut self, json: Value) -> Self {
        self.json = Some(json);
        self
    }

    pub fn accept(mut self, accept: &'a str) -> Self {
        self.accept = Some(accept);
        self
    }

    pub fn allow(mut self, allow: &'a [u16]) -> Self {
        self.allow = allow;
        self
    }

    pub fn explain(mut self, explain: Explain<'a>) -> Self {
        self.explain = Some(explain);
        self
    }

    pub fn raw(mut self) -> Self {
        self.raw = true;
        self
    }
}

/// Call GitHub and return the answer, or refuse with a message that names the next step.
///
/// A 2xx answer is returned. So is any status listed in `allow`, for a call where that status is an
/// answer and not a failure - a tag that does not exist, for one. Everything else is refused.
///
/// `path` starts with `/` and is built only from `parse_repo`, `ref_path`, `file_path` and numbers, so it
/// needs no further encoding. No query string: use `query`. A path that breaks this is a bug, and panics.
pub async fn github(
    ctx: &Ctx,
    method: &str,
    path: &str,
    options: GitHubOptions<'_>,
) -> Result<ApiAnswer, Refused> {
    if !path.starts_with('/') || path.contains('?') || path.contains('#') {
        panic!("github(): path must start with '/' and carry no query or fragment, got '{path}'.");
    }

    let token = require_token(ctx)?;
    let mut url = reqwest::Url::parse(&format!("{API_BASE}{path}"))
        .expect("an API path from address.rs parses");
    let pairs: Vec<(&str, String)> = options
        .query
        .iter()
        .filter(|(_, value)| !value.is_null() && value.as_str() != Some(""))
        .map(|(key, value)| (*key, js_string(value)))
        .collect();
    if !pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in &pairs {
            query.append_pair(key, value);
        }
    }

    let accept = options.accept.unwrap_or(JSON_ACCEPT);
    let answer = call_api(
        ctx,
        url.as_str(),
        CallOptions {
            method: method.to_string(),
            token: token.clone(),
            json: options.json,
            raw: options.raw,
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {token}")),
                ("Accept".to_string(), accept.to_string()),
                ("X-GitHub-Api-Version".to_string(), API_VERSION.to_string()),
                (
                    "User-Agent".to_string(),
                    format!("lxagents-mcp-github/{VERSION}"),
                ),
            ],
        },
    )
    .await?;

    if (200..300).contains(&answer.status) || options.allow.contains(&answer.status) {
        return Ok(answer);
    }
    let what = options.what.as_deref().unwrap_or("complete the request");
    Err(failure(
        answer.status,
        &answer.body,
        &answer.headers,
        what,
        options.explain,
        &token,
    ))
}
