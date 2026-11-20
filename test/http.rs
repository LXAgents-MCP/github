//! Ported from test/http.test.ts: the HTTP transport over real sockets on loopback, with `reqwest` as
//! the client and a mock standing in for GitHub.
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use lxagents_github::config::config::{DEFAULT_HOST, DEFAULT_PORT, LOCAL_HOST, MAX_BODY_BYTES};
use lxagents_github::config::http_settings::{HttpSettings, http_settings};
use lxagents_github::config::version::{SERVER_NAME, VERSION};
use lxagents_github::routes::http::{
    HttpOptions, RunningServer, local_request_problem, start_http_server,
};
use lxagents_github::services::context::{ApiRequest, Backend, Ctx, RawResponse, TransportError};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::helpers::{
    TOKEN, TestEnv, World, assert_matches, assert_no_match, reply, with_token, without_token,
};
use crate::surface::tool_names;

const BIN: &str = env!("CARGO_BIN_EXE_lxagents-github");

fn access() -> String {
    format!("{:9<64}", "f00dfacecafe0123456789abcdef")
}

/// A running server on a free loopback port, guarded by `token` (empty: a local install).
struct Served {
    base: String,
    port: u16,
    running: Option<RunningServer>,
}

impl Drop for Served {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            tokio::spawn(running.close());
        }
    }
}

async fn serve_ctx(ctx: Ctx, token: &str) -> Served {
    let running = start_http_server(
        Arc::new(ctx),
        HttpOptions {
            port: 0,
            host: "127.0.0.1".into(),
            auth_token: token.into(),
        },
    )
    .await
    .unwrap();
    let port = running.port;
    Served {
        base: format!("http://127.0.0.1:{port}"),
        port,
        running: Some(running),
    }
}

async fn serve(world: &World) -> Served {
    serve_ctx(world.ctx.clone(), &access()).await
}

async fn serve_local(world: &World) -> Served {
    serve_ctx(world.ctx.clone(), "").await
}

struct Sent {
    status: u16,
    headers: reqwest::header::HeaderMap,
    text: String,
    json: Value,
}

impl Sent {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

const JSON_RPC: &[(&str, &str)] = &[
    ("Content-Type", "application/json"),
    ("Accept", "application/json, text/event-stream"),
];

async fn send(
    base: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: impl Into<reqwest::Body>,
) -> Sent {
    let client = reqwest::Client::new();
    let mut request = client.request(
        reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
        format!("{base}{path}"),
    );
    for (k, v) in headers {
        request = request.header(*k, *v);
    }
    let response = request.body(body).send().await.unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let text = response.text().await.unwrap_or_default();
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Sent {
        status,
        headers,
        text,
        json,
    }
}

async fn get(base: &str, path: &str) -> Sent {
    send(base, "GET", path, &[], "").await
}

async fn rpc(base: &str, bearer: Option<&str>, method: &str, params: Value) -> Sent {
    let auth = bearer.map(|b| format!("Bearer {b}"));
    let mut headers = JSON_RPC.to_vec();
    if let Some(auth) = &auth {
        headers.push(("Authorization", auth));
    }
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    send(base, "POST", "/mcp", &headers, body).await
}

fn read_repo(name: &str) -> Value {
    json!({"name": "repo_read", "arguments": {"repo": format!("o/{name}")}})
}

fn names(answer: &Sent) -> Vec<String> {
    let mut names: Vec<String> = answer.json["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tools: {}", answer.text))
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/* ------------------------------------------------------------------ the routes */

#[tokio::test]
async fn get_healthz_is_open_and_says_nothing_but_who_is_there() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let answer = get(&s.base, "/healthz").await;
    assert_eq!(answer.status, 200);
    assert_eq!(
        answer.json,
        json!({"status": "ok", "server": SERVER_NAME, "version": VERSION})
    );
}

#[tokio::test]
async fn the_exemption_is_exactly_get_healthz() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    for (method, path) in [
        ("HEAD", "/healthz"),
        ("GET", "/healthz/"),
        ("POST", "/healthz"),
        ("GET", "/healthz2"),
    ] {
        assert_eq!(
            send(&s.base, method, path, &[], "").await.status,
            401,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn a_request_without_the_access_token_is_a_401_in_the_json_rpc_envelope() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let missing = send(&s.base, "POST", "/mcp", JSON_RPC, "this is not json").await;
    assert_eq!(
        missing.status, 401,
        "the token is checked before the body is parsed"
    );
    assert_eq!(
        missing.header("www-authenticate"),
        Some("Bearer realm=\"mcp\"")
    );
    assert_eq!(
        missing.json,
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32001, "message": "Unauthorized: send the server's access token as Authorization: Bearer <token>"}})
    );
    for authorization in ["Basic dXNlcjpwYXNz", "Bearer", "Bearer a b", "Token abc"] {
        let mut headers = JSON_RPC.to_vec();
        headers.push(("Authorization", authorization));
        let wrong = send(&s.base, "POST", "/mcp", &headers, "{}").await;
        assert_eq!(wrong.status, 401, "{authorization}");
        assert_eq!(
            wrong.header("www-authenticate"),
            Some("Bearer realm=\"mcp\""),
            "{authorization}"
        );
    }
}

#[tokio::test]
async fn a_wrong_token_is_a_401_with_invalid_token_and_never_echoes_either_token() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let a = access();
    let upper: String = a
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { 'Z' })
        .collect();
    // "tokén" cannot go in a header as UTF-8 through reqwest's checked API; its bytes are sent instead.
    for token in [
        "not-the-token".to_string(),
        a[..a.len() - 1].to_string(),
        format!("{a}x"),
        upper,
        "a".repeat(2000),
        TOKEN.to_string(),
    ] {
        let auth = format!("Bearer {token}");
        let mut headers = JSON_RPC.to_vec();
        headers.push(("Authorization", &auth));
        let answer = send(&s.base, "POST", "/mcp", &headers, "{}").await;
        assert_eq!(answer.status, 401, "{}", &token[..20.min(token.len())]);
        assert_eq!(
            answer.header("www-authenticate"),
            Some("Bearer realm=\"mcp\", error=\"invalid_token\"")
        );
        assert_eq!(
            answer.json,
            json!({"jsonrpc": "2.0", "error": {"code": -32001, "message": "Unauthorized: the token is not valid"}, "id": null})
        );
        assert!(!answer.text.contains(&a));
        assert!(!answer.text.contains(&token));
    }
    let client = reqwest::Client::new();
    let answer = client
        .post(format!("{}/mcp", s.base))
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            reqwest::header::HeaderValue::from_bytes("Bearer tokén".as_bytes()).unwrap(),
        )
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 401);
}

#[tokio::test]
async fn a_caller_without_the_token_cannot_learn_which_routes_exist() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    assert_eq!(get(&s.base, "/nope").await.status, 401);
    assert_eq!(get(&s.base, "/mcp").await.status, 401);
    assert_eq!(send(&s.base, "DELETE", "/mcp", &[], "").await.status, 401);
}

#[tokio::test]
async fn with_the_token_an_unknown_path_is_a_404_and_a_method_other_than_post_is_a_405() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let auth = format!("Bearer {}", access());
    let mut headers = JSON_RPC.to_vec();
    headers.push(("Authorization", &auth));
    let missing = send(&s.base, "GET", "/nope", &headers, "").await;
    assert_eq!(missing.status, 404);
    assert_eq!(missing.json["error"]["message"], "Not found");
    for method in ["GET", "DELETE", "PUT"] {
        let refused = send(&s.base, method, "/mcp", &headers, "").await;
        assert_eq!(refused.status, 405, "{method}");
        assert_eq!(refused.header("allow"), Some("POST"));
    }
}

#[tokio::test]
async fn a_body_that_is_not_json_is_a_400_and_one_over_the_limit_is_a_413() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let auth = format!("Bearer {}", access());
    let mut headers = JSON_RPC.to_vec();
    headers.push(("Authorization", &auth));
    let malformed = send(&s.base, "POST", "/mcp", &headers, "{not json").await;
    assert_eq!(malformed.status, 400);
    assert_eq!(malformed.json["error"]["code"], -32700);
    let oversized = send(
        &s.base,
        "POST",
        "/mcp",
        &headers,
        vec![b'a'; MAX_BODY_BYTES + 1],
    )
    .await;
    assert_eq!(oversized.status, 413);
}

/* ------------------------------------------------------------------ the tools, with the access token */

#[tokio::test]
async fn tools_list_over_http_lists_exactly_the_surface() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let answer = rpc(&s.base, Some(&access()), "tools/list", json!({})).await;
    assert_eq!(answer.status, 200, "{}", answer.text);
    assert_eq!(names(&answer), tool_names());
}

#[tokio::test]
async fn github_is_called_with_github_api_key_and_the_access_token_never_reaches_it() {
    let world = with_token(vec![reply(404, json!({"message": "Not Found"}))]);
    let s = serve(&world).await;
    let answer = rpc(&s.base, Some(&access()), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200, "{}", answer.text);
    assert_eq!(answer.json["result"]["isError"], true);
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r");
    assert_eq!(
        calls[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert!(!format!("{calls:?}").contains(&access()));
}

#[tokio::test]
async fn with_the_access_token_but_no_github_key_a_tool_is_refused_and_github_is_not_called() {
    let world = without_token(vec![]);
    let s = serve(&world).await;
    let answer = rpc(&s.base, Some(&access()), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.json["result"]["isError"], true);
    assert_matches(
        answer.json["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
        "GITHUB_API_KEY is not set",
    );
    assert!(world.calls().is_empty());
}

#[tokio::test]
async fn a_request_without_the_access_token_never_reaches_github_even_when_the_key_is_set() {
    let world = with_token(vec![]);
    let s = serve(&world).await;
    assert_eq!(
        rpc(&s.base, None, "tools/call", read_repo("r"))
            .await
            .status,
        401
    );
    assert_eq!(
        rpc(&s.base, Some(TOKEN), "tools/call", read_repo("r"))
            .await
            .status,
        401
    );
    assert!(world.calls().is_empty());
}

/// A GitHub that answers 404 after a random short wait, recording the path and the credential.
#[derive(Default)]
struct Slow(Mutex<HashMap<String, Option<String>>>);

impl Backend for Slow {
    fn send(&self, request: ApiRequest) -> BoxFuture<'_, Result<RawResponse, TransportError>> {
        Box::pin(async move {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos();
            tokio::time::sleep(Duration::from_millis(5 + (nanos % 30) as u64)).await;
            let path = reqwest::Url::parse(&request.url)
                .unwrap()
                .path()
                .to_string();
            self.0
                .lock()
                .unwrap()
                .insert(path, request.header("Authorization").map(str::to_string));
            Ok(RawResponse {
                status: 404,
                headers: Default::default(),
                body: br#"{"message":"Not Found"}"#.to_vec(),
            })
        })
    }
}

#[tokio::test]
async fn requests_that_overlap_all_reach_github_with_the_key_and_none_with_an_access_token() {
    let env = Arc::new(TestEnv::default());
    env.set("GITHUB_API_KEY", TOKEN);
    let slow = Arc::new(Slow::default());
    let ctx = Ctx {
        env,
        http: slow.clone(),
        log: Arc::new(|_: &str| {}),
    };
    let s = serve_ctx(ctx, &access()).await;
    let names: Vec<String> = (0..8).map(|i| format!("r{i}")).collect();
    let a = access();
    let answers = futures::future::join_all(
        names
            .iter()
            .map(|n| rpc(&s.base, Some(&a), "tools/call", read_repo(n))),
    )
    .await;
    assert!(answers.iter().all(|x| x.status == 200));
    let seen = slow.0.lock().unwrap();
    for name in &names {
        assert_eq!(
            seen.get(&format!("/repos/o/{name}")).cloned().flatten(),
            Some(format!("Bearer {TOKEN}")),
            "{name}"
        );
    }
}

#[tokio::test]
async fn github_rejecting_the_key_names_github_api_key_and_neither_secret_reaches_the_caller_or_the_log()
 {
    let world = with_token(vec![reply(
        401,
        json!({"message": format!("Bad credentials for {TOKEN}")}),
    )]);
    let s = serve(&world).await;
    let answer = rpc(&s.base, Some(&access()), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200);
    assert_matches(
        answer.json["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
        r"GitHub rejected the token \(401\)\. Check that GITHUB_API_KEY holds a valid token",
    );
    let logs = world.logs().join("");
    for secret in [TOKEN.to_string(), access()] {
        assert!(!answer.text.contains(&secret));
        assert!(!logs.contains(&secret));
    }
}

/* ------------------------------------------------------------------ starting */

async fn start_err(host: &str, token: &str) -> String {
    let world = without_token(vec![]);
    match start_http_server(
        Arc::new(world.ctx.clone()),
        HttpOptions {
            port: 0,
            host: host.into(),
            auth_token: token.into(),
        },
    )
    .await
    {
        Ok(running) => {
            running.close().await;
            panic!("started on {host}")
        }
        Err(e) => e,
    }
}

#[tokio::test]
async fn start_http_server_will_not_listen_without_an_access_token_of_32_characters() {
    assert_eq!(start_err("0.0.0.0", "").await, "MCP_AUTH_TOKEN is not set");
    assert_matches(
        &start_err("0.0.0.0", "short").await,
        "MCP_AUTH_TOKEN is 5 characters, and it must be at least 32",
    );
}

fn settings(vars: &[(&str, &str)]) -> Result<HttpSettings, String> {
    let env = TestEnv::default();
    for (k, v) in vars {
        env.set(k, v);
    }
    http_settings(&env)
}

fn at(port: u16, host: &str) -> Result<HttpSettings, String> {
    Ok(HttpSettings {
        port,
        host: host.into(),
    })
}

#[test]
fn http_settings_with_an_access_token_defaults_to_port_3000_on_every_interface() {
    let a = access();
    let t = ("MCP_AUTH_TOKEN", a.as_str());
    assert_eq!(settings(&[t]), at(3000, "0.0.0.0"));
    assert_eq!(
        settings(&[t, ("PORT", ""), ("HOST", "")]),
        at(3000, "0.0.0.0")
    );
    assert_eq!(
        settings(&[t, ("PORT", "   "), ("HOST", "   ")]),
        at(3000, "0.0.0.0")
    );
    assert_eq!(DEFAULT_PORT, 3000);
    assert_eq!(DEFAULT_HOST, "0.0.0.0");
}

#[test]
fn http_settings_without_an_access_token_defaults_to_this_machine_only() {
    assert_eq!(settings(&[]), at(3000, "127.0.0.1"));
    assert_eq!(
        settings(&[("PORT", ""), ("HOST", "")]),
        at(3000, "127.0.0.1")
    );
    assert_eq!(
        settings(&[("MCP_AUTH_TOKEN", "   ")]),
        at(3000, "127.0.0.1")
    );
    assert_eq!(LOCAL_HOST, "127.0.0.1");
    assert_eq!(settings(&[("HOST", "0.0.0.0")]), at(3000, "0.0.0.0"));
}

#[test]
fn http_settings_lets_port_and_host_override_accepts_0_and_refuses_a_port_that_is_not_a_port() {
    assert_eq!(
        settings(&[("PORT", " 8080 "), ("HOST", " 127.0.0.1 ")]),
        at(8080, "127.0.0.1")
    );
    assert_eq!(
        settings(&[("PORT", "0"), ("MCP_AUTH_TOKEN", &access())]),
        at(0, "0.0.0.0")
    );
    for bad in ["abc", "70000", "-1", "80.5", "1e9"] {
        assert_matches(
            &settings(&[("PORT", bad)]).unwrap_err(),
            &format!("PORT must be a whole number from 0 to 65535, got '{bad}'"),
        );
    }
}

/* ------------------------------------------------------------------ the process */

struct Proc {
    child: Child,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
}

impl Proc {
    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }
    fn stdout(&self) -> String {
        self.stdout.lock().unwrap().clone()
    }

    /// Wait until stderr matches, for up to ten seconds.
    async fn wait_for(&self, pattern: &str) -> String {
        let re = regex::Regex::new(pattern).unwrap();
        for _ in 0..200 {
            let text = self.stderr();
            if re.is_match(&text) {
                return text;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("no startup line: {}", self.stderr());
    }

    /// The exit code, or `None` if it is still running after ten seconds.
    async fn exit_within(&mut self) -> Option<i32> {
        let code = tokio::time::timeout(Duration::from_secs(10), self.child.wait())
            .await
            .ok()
            .map(|s| s.unwrap().code());
        // Let the readers drain what was written last.
        tokio::time::sleep(Duration::from_millis(50)).await;
        code.flatten()
            .or(if code.is_some() { Some(-1) } else { None })
    }

    fn signal(&self, name: &str) {
        let pid = self.child.id().unwrap().to_string();
        assert!(
            std::process::Command::new("kill")
                .args([&format!("-{name}"), &pid])
                .status()
                .unwrap()
                .success()
        );
    }
}

fn collect(stream: impl tokio::io::AsyncRead + Unpin + Send + 'static) -> Arc<Mutex<String>> {
    let text = Arc::new(Mutex::new(String::new()));
    let sink = text.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut s = sink.lock().unwrap();
            s.push_str(&line);
            s.push('\n');
        }
    });
    text
}

/// The binary with exactly these variables on top of ours, minus both secrets and the HTTP ones.
fn start_process(env: &[(&str, &str)]) -> Proc {
    let mut command = Command::new(BIN);
    for name in [
        "GITHUB_API_KEY",
        "MCP_AUTH_TOKEN",
        "MCP_TRANSPORT",
        "PORT",
        "HOST",
    ] {
        command.env_remove(name);
    }
    command
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().unwrap();
    let stdout = collect(child.stdout.take().unwrap());
    let stderr = collect(child.stderr.take().unwrap());
    Proc {
        child,
        stdout,
        stderr,
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn mcp_transport_http_serves_http_from_a_real_process_and_stops_on_sigterm() {
    let key = "ghp_process_key_value_9876";
    let a = access();
    let mut run = start_process(&[
        ("MCP_TRANSPORT", "http"),
        ("PORT", "0"),
        ("HOST", "127.0.0.1"),
        ("MCP_AUTH_TOKEN", &a),
        ("GITHUB_API_KEY", key),
    ]);
    let stderr = run
        .wait_for(r"serving over http on http://127\.0\.0\.1:\d+/mcp")
        .await;
    let base = regex::Regex::new(r"serving over http on (http://127\.0\.0\.1:\d+)/mcp")
        .unwrap()
        .captures(&stderr)
        .unwrap()[1]
        .to_string();

    assert_eq!(get(&base, "/healthz").await.status, 200);
    assert_eq!(rpc(&base, None, "tools/list", json!({})).await.status, 401);
    assert_eq!(
        rpc(&base, Some(key), "tools/list", json!({})).await.status,
        401
    );
    assert_eq!(
        rpc(&base, Some(&a), "tools/list", json!({})).await.status,
        200
    );

    run.signal("TERM");
    assert_eq!(run.exit_within().await, Some(0));
    assert_eq!(run.stdout(), "");
    let stderr = run.stderr();
    assert!(!stderr.contains(&a) && !stderr.contains(key));
    assert_no_match(&stderr, "GITHUB_API_KEY is not set");
}

#[tokio::test]
async fn with_no_host_a_real_process_listens_on_every_interface_and_with_host_only_there() {
    let a = access();
    for (host, said) in [
        (None, r"serving over http on :\d+/mcp \(all interfaces\)"),
        (
            Some("127.0.0.1"),
            r"serving over http on http://127\.0\.0\.1:\d+/mcp",
        ),
    ] {
        let port = free_port().to_string();
        let mut env = vec![
            ("MCP_TRANSPORT", "http"),
            ("PORT", port.as_str()),
            ("MCP_AUTH_TOKEN", a.as_str()),
        ];
        if let Some(h) = host {
            env.push(("HOST", h));
        }
        let mut run = start_process(&env);
        let stderr = run.wait_for("serving over http on").await;
        assert_matches(&stderr, said);
        assert_eq!(
            get(&format!("http://127.0.0.1:{port}"), "/healthz")
                .await
                .status,
            200
        );
        run.signal("TERM");
        run.exit_within().await;
    }
}

#[tokio::test]
async fn http_without_a_github_key_starts_and_says_so() {
    let a = access();
    let mut run = start_process(&[
        ("MCP_TRANSPORT", "http"),
        ("PORT", "0"),
        ("MCP_AUTH_TOKEN", &a),
    ]);
    let stderr = run.wait_for("serving over http on").await;
    assert_matches(
        &stderr,
        "GITHUB_API_KEY is not set in this process's environment: every tool will refuse until it is",
    );
    run.signal("TERM");
    run.exit_within().await;
}

#[tokio::test]
async fn http_other_machines_can_reach_will_not_start_without_a_usable_access_token() {
    let cases: &[(&[(&str, &str)], &str)] = &[
        (&[("HOST", "0.0.0.0")], "MCP_AUTH_TOKEN is not set"),
        (
            &[("HOST", "0.0.0.0"), ("MCP_AUTH_TOKEN", "   ")],
            "MCP_AUTH_TOKEN is not set",
        ),
        (&[("HOST", "192.0.2.1")], "MCP_AUTH_TOKEN is not set"),
        (
            &[("MCP_AUTH_TOKEN", "too-short")],
            "MCP_AUTH_TOKEN is 9 characters, and it must be at least 32",
        ),
        (
            &[("HOST", "127.0.0.1"), ("MCP_AUTH_TOKEN", "too-short")],
            "MCP_AUTH_TOKEN is 9 characters, and it must be at least 32",
        ),
        (
            &[
                ("HOST", "0.0.0.0"),
                ("GITHUB_API_KEY", "ghp_only_the_key_1234"),
            ],
            "MCP_AUTH_TOKEN is not set",
        ),
    ];
    for (env, reason) in cases {
        let mut all = vec![("MCP_TRANSPORT", "http"), ("PORT", "0")];
        all.extend_from_slice(env);
        let mut run = start_process(&all);
        assert_eq!(run.exit_within().await, Some(1), "{env:?}");
        let stderr = run.stderr();
        assert_matches(&stderr, reason);
        assert_matches(&stderr, "will not start the HTTP transport");
        assert_matches(&stderr, "openssl rand -hex 32");
        assert_matches(
            &stderr,
            "A local install needs no access token: leave HOST unset",
        );
        assert_eq!(run.stdout(), "");
        assert_no_match(&stderr, "serving over http");
        assert!(!stderr.contains("too-short"));
    }
}

#[tokio::test]
async fn an_unknown_mcp_transport_or_a_port_that_is_not_a_port_exits_2() {
    let a = access();
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (
            vec![("MCP_TRANSPORT", "carrier-pigeon")],
            "MCP_TRANSPORT must be stdio or http, got 'carrier-pigeon'",
        ),
        (
            vec![
                ("MCP_TRANSPORT", "http"),
                ("PORT", "http"),
                ("MCP_AUTH_TOKEN", &a),
            ],
            "PORT must be a whole number from 0 to 65535, got 'http'",
        ),
        (
            vec![
                ("MCP_TRANSPORT", "http"),
                ("PORT", "70000"),
                ("MCP_AUTH_TOKEN", &a),
            ],
            "PORT must be a whole number from 0 to 65535, got '70000'",
        ),
    ];
    for (env, reason) in cases {
        let mut run = start_process(&env);
        assert_eq!(run.exit_within().await, Some(2), "{env:?}");
        assert_matches(&run.stderr(), reason);
        assert_eq!(run.stdout(), "");
    }
}

/* ------------------------------------------------------------------ a local install: no access token */

#[tokio::test]
async fn start_http_server_takes_no_access_token_only_on_loopback() {
    for host in ["0.0.0.0", "192.0.2.1"] {
        assert_eq!(
            start_err(host, "").await,
            "MCP_AUTH_TOKEN is not set",
            "{host}"
        );
    }
    let world = without_token(vec![]);
    let s = serve_local(&world).await;
    assert_eq!(get(&s.base, "/healthz").await.status, 200);
}

fn list_body() -> String {
    json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}).to_string()
}

#[tokio::test]
async fn a_local_install_serves_a_request_addressed_to_loopback_without_any_token() {
    let world = without_token(vec![]);
    let s = serve_local(&world).await;
    let port = s.port;
    // reqwest always sends a Host; `None` is its own, 127.0.0.1:<port>.
    for host in [
        None,
        Some(format!("localhost:{port}")),
        Some("localhost".into()),
        Some(format!("127.0.0.1:{port}")),
        Some(format!("[::1]:{port}")),
    ] {
        let mut headers = JSON_RPC.to_vec();
        if let Some(h) = &host {
            headers.push(("Host", h));
        }
        let answer = send(&s.base, "POST", "/mcp", &headers, list_body()).await;
        assert_eq!(answer.status, 200, "{host:?}");
        assert_eq!(names(&answer), tool_names());
    }
    let mut headers = JSON_RPC.to_vec();
    headers.push(("Origin", "http://localhost:5173"));
    assert_eq!(
        send(&s.base, "POST", "/mcp", &headers, list_body())
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn a_local_install_refuses_a_request_a_web_page_could_have_rebound_to_it() {
    let world = without_token(vec![]);
    let s = serve_local(&world).await;
    let port = s.port;
    let cases = [
        ("Host", "evil.example".to_string()),
        ("Host", format!("evil.example:{port}")),
        ("Host", format!("localhost.evil.example:{port}")),
        ("Host", format!("localhost@evil.example:{port}")),
        ("Host", format!("127.0.0.1.nip.io:{port}")),
        ("Origin", "http://evil.example".to_string()),
        ("Origin", format!("http://evil.example:{port}")),
        ("Origin", "null".to_string()),
        ("Origin", "file://".to_string()),
    ];
    for (name, value) in &cases {
        let mut headers = JSON_RPC.to_vec();
        headers.push((name, value));
        let answer = send(&s.base, "POST", "/mcp", &headers, "this is not json").await;
        assert_eq!(answer.status, 403, "{name}: {value}");
        assert_eq!(answer.json["error"]["code"], -32001);
        assert_matches(
            answer.json["error"]["message"].as_str().unwrap(),
            "^Forbidden: this server has no access token",
        );
    }
    assert_eq!(
        send(&s.base, "GET", "/healthz", &[("Host", "evil.example")], "")
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn a_local_install_still_routes_like_a_server_install_once_addressed_to_loopback() {
    let world = without_token(vec![]);
    let s = serve_local(&world).await;
    assert_eq!(get(&s.base, "/mcp").await.status, 405);
    assert_eq!(
        send(&s.base, "POST", "/other", JSON_RPC, "{}").await.status,
        404
    );
    assert_eq!(
        send(&s.base, "POST", "/mcp", JSON_RPC, "this is not json")
            .await
            .status,
        400
    );
}

#[test]
fn local_request_problem_reads_host_whole_and_origin_only_as_a_web_origin() {
    assert_eq!(local_request_problem(Some("127.0.0.1:3000"), None), None);
    assert_eq!(
        local_request_problem(Some("[::1]:3000"), Some("https://localhost")),
        None
    );
    assert_matches(
        local_request_problem(None, None).unwrap_or(""),
        "addressed to localhost",
    );
    assert_matches(
        local_request_problem(Some("127.0.0.1:3000/evil"), None).unwrap_or(""),
        "addressed to localhost",
    );
    assert_matches(
        local_request_problem(Some("127.0.0.1:3000"), Some("ftp://localhost")).unwrap_or(""),
        "another origin",
    );
    assert_matches(
        local_request_problem(Some("127.0.0.1:3000"), Some("not a url")).unwrap_or(""),
        "another origin",
    );
}

#[tokio::test]
async fn with_neither_mcp_auth_token_nor_host_a_real_process_is_a_local_install() {
    let port = free_port();
    let p = port.to_string();
    let mut run = start_process(&[
        ("MCP_TRANSPORT", "http"),
        ("PORT", &p),
        ("GITHUB_API_KEY", "ghp_process_key_value_9876"),
    ]);
    let stderr = run.wait_for("local install").await;
    assert_matches(
        &stderr,
        &format!(r"serving over http on http://127\.0\.0\.1:{port}/mcp"),
    );
    assert_matches(
        &stderr,
        "local install: MCP_AUTH_TOKEN is not set, so only this machine can reach the server",
    );
    assert_no_match(&stderr, "all interfaces");
    assert_eq!(
        rpc(
            &format!("http://127.0.0.1:{port}"),
            None,
            "tools/list",
            json!({})
        )
        .await
        .status,
        200
    );
    run.signal("TERM");
    assert_eq!(run.exit_within().await, Some(0));
    assert_eq!(run.stdout(), "");
}
