//! Everything a tool reads from outside its arguments: the environment and the network.
//!
//! A handler never reads `std::env` or opens a connection itself. It is handed a `Ctx`, which in the
//! server reads the process environment at call time and sends requests with `reqwest`, and in a test
//! is a map and a queue of canned answers. That keeps the two secrets read per call, as they always
//! were, and lets tests run in parallel without touching process state.
use std::collections::BTreeMap;
use std::sync::Arc;

use futures::future::BoxFuture;

use super::client::ReqwestBackend;

/// Where variables come from.
pub trait Env: Send + Sync {
    fn var(&self, name: &str) -> Option<String>;
}

/// The process environment, read when asked.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// One request to a hosting API, as `callApi` hands it to the network.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl ApiRequest {
    /// The value of a header, by name in any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What the network answered: the status, the headers lowercased, and the whole body.
#[derive(Debug, Clone, Default)]
pub struct RawResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// Why no answer arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Nothing within the time allowed.
    Timeout,
    /// The answer grew past the size the client reads.
    TooLarge,
    /// No connection, a refused one, a TLS failure: the reason, as the network library put it.
    Network(String),
}

/// Sends one request. The server's is `ReqwestBackend`; tests queue answers instead.
pub trait Backend: Send + Sync {
    fn send(&self, request: ApiRequest) -> BoxFuture<'_, Result<RawResponse, TransportError>>;
}

/// Where diagnostics go. The server's writes to stderr; a test keeps the lines to read them.
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

/// What a tool runs in.
#[derive(Clone)]
pub struct Ctx {
    pub env: Arc<dyn Env>,
    pub http: Arc<dyn Backend>,
    pub log: LogSink,
}

impl Ctx {
    /// The server's context: the process environment, the real network, and stderr.
    pub fn production() -> Self {
        Ctx {
            env: Arc::new(ProcessEnv),
            http: Arc::new(ReqwestBackend::new()),
            log: Arc::new(crate::utils::log::log),
        }
    }

    /// Write one diagnostic line.
    pub fn log(&self, message: &str) {
        (self.log)(message)
    }

    /// A variable, trimmed, or an empty string.
    pub fn var(&self, name: &str) -> String {
        self.env.var(name).unwrap_or_default().trim().to_string()
    }
}
