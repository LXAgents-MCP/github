//! The HTTP transport: `POST /mcp`, and `GET /healthz`, served by axum.
//!
//! Two kinds of install, told apart by where the server listens:
//!
//! - **A server install** listens where other machines can reach it (`0.0.0.0`, or any `HOST` that is not
//!   loopback). Every route but `GET /healthz` needs `Authorization: Bearer <token>`, and the token is this
//!   server's own access token, `MCP_AUTH_TOKEN` (`src/services/auth.rs`). It is a secret between the client
//!   and this server: it is checked before anything else is read - before the body is read and before a
//!   path is matched, so a caller without it cannot make this server read 4 MB or find out which routes
//!   exist - and it is never sent to GitHub. The server refuses to start without an access token, rather
//!   than serving an open door.
//! - **A local install** listens on loopback only, and may leave `MCP_AUTH_TOKEN` unset: nothing beyond this
//!   machine can reach it. With no `HOST` and no token it is what the server does. A web page open on this
//!   machine still can, through DNS rebinding, so without a token a request must say it is addressed to
//!   loopback in `Host`, and in `Origin` when it sends one, before anything is read. A token that is set is
//!   required here as well.
//!
//! The calls to GitHub are made with `GITHUB_API_KEY` from this process's environment, whoever the caller
//! is. Nothing is stored between requests: each one gets its own `GithubServer`, through `rmcp`'s
//! stateless streamable HTTP service, answered as plain JSON.
//!
//! Diagnostics go to stderr, as everywhere in this server.
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Method, Response, header};
use axum::response::IntoResponse;
use futures::FutureExt;
use http_body_util::{BodyExt, Limited};
use regex::Regex;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tower::ServiceExt;

use super::server::GithubServer;
use crate::config::config::{AUTH_ENV, MAX_BODY_BYTES, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV};
use crate::config::http_settings::http_settings;
use crate::config::version::{SERVER_NAME, VERSION};
use crate::services::auth::{
    auth_token_problem, bearer_credential, configured_api_key, configured_auth_token,
    credential_matches, digest, is_loopback_host,
};
use crate::services::context::Ctx;
use crate::utils::http::{rpc_error, unauthorized};
use crate::utils::redact::redact;

static HOST_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\[[0-9a-f:.]+\]|[a-z0-9.-]+)(?::\d{1,5})?$").unwrap());

/// Why a request to a local install - loopback, no access token - is refused, or `None` when it is not.
///
/// Without a token, what keeps a web page from driving this server is that a browser always says where a
/// request is going: `Host` is the name the page used, and `Origin` is the page itself. A rebound name or
/// another site shows up in one of them. A client that is not a browser sends a loopback `Host` and no
/// `Origin`, and passes. The `Host` value is matched whole, as `name` or `name:port`, so nothing in it can
/// make a URL parser read a different host.
pub fn local_request_problem(host: Option<&str>, origin: Option<&str>) -> Option<&'static str> {
    let addressed = HOST_HEADER
        .captures(host.unwrap_or(""))
        .map(|caps| is_loopback_host(&caps[1]))
        .unwrap_or(false);
    if !addressed {
        return Some(
            "Forbidden: this server has no access token, so it only answers requests addressed to localhost, 127.0.0.1 or [::1]",
        );
    }
    if let Some(origin) = origin {
        let name = reqwest::Url::parse(origin)
            .ok()
            .filter(|url| url.scheme() == "http" || url.scheme() == "https")
            .and_then(|url| url.host_str().map(str::to_string))
            .unwrap_or_default();
        if !is_loopback_host(&name) {
            return Some(
                "Forbidden: this server has no access token, so it does not answer a web page from another origin",
            );
        }
    }
    None
}

type McpService = StreamableHttpService<GithubServer, NeverSessionManager>;

/// What every request is answered with.
#[derive(Clone)]
struct AppState {
    ctx: Arc<Ctx>,
    /// `digest()` of the access token a request must carry, or `None` for a local install without one,
    /// where the request must be addressed to loopback instead.
    expected: Option<[u8; 32]>,
    auth_token: String,
    mcp: McpService,
}

fn header_text(request: &Request, name: header::HeaderName) -> Option<&str> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

/// Answer one request.
async fn route(state: &AppState, request: Request) -> Response<Body> {
    let path = request.uri().path().to_string();

    // The one open route, and exact: the method and the path, nothing that merely starts with it.
    if request.method() == Method::GET && path == "/healthz" {
        let body = json!({"status": "ok", "server": SERVER_NAME, "version": VERSION}).to_string();
        return ([(header::CONTENT_TYPE, "application/json")], body).into_response();
    }

    match &state.expected {
        None => {
            let host = header_text(&request, header::HOST);
            let origin = header_text(&request, header::ORIGIN);
            if let Some(problem) = local_request_problem(host, origin) {
                return rpc_error(403, -32001, problem, &[]);
            }
        }
        Some(expected) => {
            let Some(credential) = bearer_credential(header_text(&request, header::AUTHORIZATION))
            else {
                return unauthorized(false);
            };
            if !credential_matches(&credential, expected) {
                return unauthorized(true);
            }
        }
    }

    if path != "/mcp" {
        return rpc_error(404, -32000, "Not found", &[]);
    }
    if request.method() != Method::POST {
        return rpc_error(
            405,
            -32000,
            "Method not allowed: use POST",
            &[("Allow", "POST")],
        );
    }

    let (parts, body) = request.into_parts();
    let bytes = match Limited::new(body, MAX_BODY_BYTES).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            if error
                .downcast_ref::<http_body_util::LengthLimitError>()
                .is_some()
            {
                return rpc_error(
                    413,
                    -32000,
                    "Request body too large",
                    &[("Connection", "close")],
                );
            }
            return rpc_error(400, -32700, "Parse error: the body is not JSON", &[]);
        }
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
        return rpc_error(400, -32700, "Parse error: the body is not JSON", &[]);
    }

    let forwarded = Request::from_parts(parts, Body::from(bytes));
    match state.mcp.clone().oneshot(forwarded).await {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    }
}

/// Answer one request, turning a crash into a 500 that says nothing, and a log line that names no secret.
async fn handle(State(state): State<AppState>, request: Request) -> Response<Body> {
    let presented =
        bearer_credential(header_text(&request, header::AUTHORIZATION)).unwrap_or_default();
    match AssertUnwindSafe(route(&state, request))
        .catch_unwind()
        .await
    {
        Ok(response) => response,
        Err(payload) => {
            // A crash is text nobody wrote for a reader, so it is scrubbed like any other. Any of the
            // secrets can be in the message, and none may reach the log or the caller: the GitHub key, the
            // access token, and whatever credential this request presented.
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let text = format!("HTTP handler crashed:\n{message}");
            let key = configured_api_key(state.ctx.env.as_ref());
            state.ctx.log(&redact(
                &redact(&redact(&text, &key), &state.auth_token),
                &presented,
            ));
            rpc_error(500, -32603, "Internal error", &[])
        }
    }
}

/// Where `start_http_server` listens, and the access token requests must carry.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    /// 0 asks the system for a free port.
    pub port: u16,
    pub host: String,
    /// May be empty only when `host` is loopback, a local install; otherwise it must pass
    /// `auth_token_problem`.
    pub auth_token: String,
}

/// A listening HTTP transport: where it ended up listening, and how to stop it.
pub struct RunningServer {
    pub port: u16,
    pub host: String,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl RunningServer {
    /// Stop listening, let requests in flight finish, and give up on them after 5 seconds.
    pub async fn close(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        if tokio::time::timeout(Duration::from_secs(5), &mut task)
            .await
            .is_err()
        {
            task.abort();
        }
    }
}

/// The axum app for one set of options, without a listener: what `start_http_server` serves.
pub fn app(ctx: Arc<Ctx>, auth_token: &str, local: bool) -> Router {
    let factory_ctx = ctx.clone();
    // Our own `Host` check runs first and is stricter, so the service's is off; its body cap matches ours.
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_max_request_body_bytes(MAX_BODY_BYTES)
        .disable_allowed_hosts();
    let mcp = StreamableHttpService::new(
        move || Ok(GithubServer::new(factory_ctx.clone())),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    let state = AppState {
        ctx,
        expected: if local {
            None
        } else {
            Some(digest(auth_token))
        },
        auth_token: auth_token.to_string(),
        mcp,
    };
    Router::new().fallback(handle).with_state(state)
}

/// Start listening.
pub async fn start_http_server(
    ctx: Arc<Ctx>,
    options: HttpOptions,
) -> Result<RunningServer, String> {
    let local = options.auth_token.is_empty() && is_loopback_host(&options.host);
    if !local && let Some(problem) = auth_token_problem(&options.auth_token) {
        return Err(problem);
    }

    let bind_host = options
        .host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let listener = match bind_host.parse::<std::net::IpAddr>() {
        Ok(ip) => TcpListener::bind(SocketAddr::new(ip, options.port)).await,
        Err(_) => TcpListener::bind((bind_host.as_str(), options.port)).await,
    }
    .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map(|addr| addr.port())
        .unwrap_or(options.port);

    let router = app(ctx, &options.auth_token, local);
    let (stop, mut stopped) = watch::channel(false);
    let task = tokio::spawn(async move {
        let shutdown = async move {
            let _ = stopped.wait_for(|stop| *stop).await;
        };
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await;
    });
    Ok(RunningServer {
        port,
        host: options.host,
        stop,
        task,
    })
}

/// Wait for SIGINT or SIGTERM. SIGTERM as well as SIGINT: in a container this process is PID 1, which the
/// kernel gives no default action for a signal, so without a handler `docker stop` waits out its grace
/// period and then kills it.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("a SIGTERM handler installs");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Run the HTTP transport as this process's job: read the settings, listen, and close on a signal.
/// Returns the exit code.
pub async fn run_http(ctx: Arc<Ctx>) -> i32 {
    let settings = match http_settings(ctx.env.as_ref()) {
        Ok(settings) => settings,
        Err(message) => {
            ctx.log(&message);
            return 2;
        }
    };

    // A local install - loopback, and no token - needs none. Anything else fails closed, before anything
    // listens: HTTP without an access token where other machines can reach it is an open door on a
    // network, and the only thing to do about it is not to start. A token that is set is always checked.
    let auth_token = configured_auth_token(ctx.env.as_ref());
    let local = auth_token.is_empty() && is_loopback_host(&settings.host);
    let problem = if local {
        None
    } else {
        auth_token_problem(&auth_token)
    };
    if let Some(problem) = problem {
        ctx.log(&format!(
            "{SERVER_NAME} {VERSION} will not start the HTTP transport on {}: {problem}. Other machines can reach that address, so set {AUTH_ENV} to a secret of at least {MIN_AUTH_TOKEN_LENGTH} characters, for example the output of: openssl rand -hex 32. A local install needs no access token: leave HOST unset, or set it to 127.0.0.1, and leave {AUTH_ENV} unset. stdio needs no access token.",
            settings.host
        ));
        return 1;
    }

    // Not fatal, as it is not over stdio: the server starts and a tool names the variable when it is
    // called. Said at start as well, because over HTTP nobody is watching the first call.
    if configured_api_key(ctx.env.as_ref()).is_empty() {
        ctx.log(&format!("{TOKEN_ENV} is not set in this process's environment: every tool will refuse until it is."));
    }

    let options = HttpOptions {
        port: settings.port,
        host: settings.host.clone(),
        auth_token,
    };
    let running = match start_http_server(ctx.clone(), options).await {
        Ok(running) => running,
        Err(message) => {
            ctx.log(&format!(
                "{SERVER_NAME} {VERSION} could not listen on {}:{}: {message}",
                settings.host, settings.port
            ));
            return 1;
        }
    };

    let name = if running.host.contains(':') {
        format!("[{}]", running.host)
    } else {
        running.host.clone()
    };
    let at = if running.host == "0.0.0.0" {
        format!(":{}/mcp (all interfaces)", running.port)
    } else {
        format!("http://{name}:{}/mcp", running.port)
    };
    ctx.log(&format!(
        "{SERVER_NAME} {VERSION} serving over http on {at}"
    ));
    if local {
        ctx.log(&format!(
            "local install: {AUTH_ENV} is not set, so only this machine can reach the server and no access token is asked for. Requests must be addressed to localhost, 127.0.0.1 or [::1]. To serve other machines, set {AUTH_ENV} and leave HOST unset or make it 0.0.0.0."
        ));
    } else {
        ctx.log(&format!(
            "the access token in {AUTH_ENV} is required on every route except GET /healthz."
        ));
    }

    shutdown_signal().await;
    // A second signal means the operator has stopped waiting.
    tokio::select! {
        _ = running.close() => {}
        _ = shutdown_signal() => {}
    }
    0
}
