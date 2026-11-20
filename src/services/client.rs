//! The HTTP client under every GitHub call: one request with a timeout and a size cap, and every failure
//! to get an answer turned into a `Refused` the model can read.
//!
//! Nothing here writes to stdout: on stdio, stdout is the protocol channel.
use std::collections::BTreeMap;
use std::time::Duration;

use futures::future::BoxFuture;
use serde_json::Value;

use super::context::{ApiRequest, Backend, Ctx, RawResponse, TransportError};
use crate::config::config::{API_TIMEOUT_MS, MAX_RESPONSE_BYTES};
use crate::utils::errors::Refused;
use crate::utils::redact::redact;
use crate::utils::text::take_chars;

/// What a hosting API answered: the status, the decoded body, and the headers, lowercased.
#[derive(Debug, Clone)]
pub struct ApiAnswer {
    pub status: u16,
    /// Decoded JSON, the text of a `raw` answer, the start of an answer that was not JSON, or `null` when
    /// there was no body. Callers read it defensively.
    pub body: Value,
    pub headers: BTreeMap<String, String>,
}

/// How `call_api` makes one request.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub json: Option<Value>,
    /// Only used to scrub error text; the caller puts the credential in `headers`.
    pub token: String,
    /// Keep the whole body of a successful answer as text instead of decoding JSON; an error answer is
    /// still decoded, so that its message can be read.
    pub raw: bool,
}

/// The host and port of a URL, as `new URL(url).host` gives it.
fn host_of(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) => match (parsed.host_str(), parsed.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.to_string(),
            _ => String::new(),
        },
        Err(_) => String::new(),
    }
}

/// Call a hosting API and return its status, decoded body and headers.
///
/// Every failure to get an answer - no network, a refused connection, a timeout - becomes a `Refused` the
/// model can read and act on. A non-2xx answer is not a failure here: the caller knows what each status
/// means.
pub async fn call_api(ctx: &Ctx, url: &str, options: CallOptions) -> Result<ApiAnswer, Refused> {
    let host = host_of(url);
    let mut headers = options.headers.clone();
    let body = options.json.as_ref().map(|json| {
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
        json.to_string()
    });
    let method = if options.method.is_empty() {
        "GET".to_string()
    } else {
        options.method.clone()
    };
    let request = ApiRequest {
        method,
        url: url.to_string(),
        headers,
        body,
    };

    let response = match ctx.http.send(request).await {
        Ok(response) => response,
        Err(TransportError::Timeout) => {
            return Err(Refused(format!(
                "{host} did not answer within {} seconds. Try again.",
                API_TIMEOUT_MS / 1000
            )));
        }
        Err(TransportError::TooLarge) => {
            return Err(Refused(format!(
                "The answer from {host} is larger than {} MB. Ask for less.",
                MAX_RESPONSE_BYTES / 1_000_000
            )));
        }
        Err(TransportError::Network(reason)) => {
            let reason = redact(&reason, &options.token);
            return Err(Refused(format!(
                "Could not reach {host}: {reason}. Check the network connection."
            )));
        }
    };

    let ok = (200..300).contains(&response.status);
    let text = String::from_utf8_lossy(&response.body).into_owned();
    let body = if text.is_empty() {
        Value::Null
    } else if options.raw && ok {
        Value::String(text)
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(take_chars(&text, 300)))
    };
    Ok(ApiAnswer {
        status: response.status,
        body,
        headers: response.headers,
    })
}

/// The real network: one shared `reqwest` client with rustls.
///
/// A redirect is followed up to ten times, and `reqwest` drops `Authorization` from a redirect that
/// changes host or port, as `fetch` did, so the key cannot be carried to a second host.
pub struct ReqwestBackend {
    client: reqwest::Client,
    max_bytes: usize,
}

impl ReqwestBackend {
    pub fn new() -> Self {
        Self::with_limits(Duration::from_millis(API_TIMEOUT_MS), MAX_RESPONSE_BYTES)
    }

    /// A client with its own timeout and size cap, for a test that cannot wait 30 seconds.
    pub fn with_limits(timeout: Duration, max_bytes: usize) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("the HTTP client builds with static settings");
        ReqwestBackend { client, max_bytes }
    }

    async fn fetch(&self, request: ApiRequest) -> Result<RawResponse, TransportError> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .map_err(|_| TransportError::Network(format!("invalid method {}", request.method)))?;
        let mut builder = self.client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let mut response = builder.send().await.map_err(transport_error)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_ascii_lowercase(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .fold(BTreeMap::new(), |mut map, (name, value)| {
                map.entry(name)
                    .and_modify(|existing: &mut String| {
                        existing.push_str(", ");
                        existing.push_str(&value);
                    })
                    .or_insert(value);
                map
            });
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            body.extend_from_slice(&chunk);
            if body.len() > self.max_bytes {
                return Err(TransportError::TooLarge);
            }
        }
        Ok(RawResponse {
            status,
            headers,
            body,
        })
    }
}

impl Default for ReqwestBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Why `reqwest` got no answer, with the innermost cause, which is the one that names the problem.
fn transport_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        return TransportError::Timeout;
    }
    let mut reason = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(inner) = source {
        reason = inner.to_string();
        source = inner.source();
    }
    TransportError::Network(reason)
}

impl Backend for ReqwestBackend {
    fn send(&self, request: ApiRequest) -> BoxFuture<'_, Result<RawResponse, TransportError>> {
        Box::pin(self.fetch(request))
    }
}
