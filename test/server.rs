//! Ported from test/server.test.ts. The client speaks raw JSON-RPC lines over an in-memory pipe, so
//! what is checked is exactly what goes over the wire.
use std::sync::Arc;

use futures::future::BoxFuture;
use lxagents_github::config::version::{SERVER_NAME, VERSION};
use lxagents_github::controllers;
use lxagents_github::models::args::Args;
use lxagents_github::models::tool::{Outcome, ToolSpec};
use lxagents_github::routes::server::{GithubServer, tool_result};
use lxagents_github::services::context::Ctx;
use lxagents_github::utils::errors::Refused;
use rmcp::ServiceExt;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};

use crate::helpers::{TOKEN, World, assert_matches, obj, reply, with_token, without_token};
use crate::surface::{kind_hints, kind_of, tool_names};

/// A JSON-RPC client over newline-delimited JSON.
pub struct Rpc<R, W> {
    lines: Lines<BufReader<R>>,
    writer: W,
    next: i64,
    pub initialized: Value,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Rpc<R, W> {
    /// Connect and complete the handshake.
    pub async fn connect(reader: R, writer: W) -> Self {
        let mut rpc = Rpc {
            lines: BufReader::new(reader).lines(),
            writer,
            next: 0,
            initialized: Value::Null,
        };
        rpc.initialized = rpc
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "test", "version": "0.0.0"}
                }),
            )
            .await;
        rpc.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
        rpc
    }

    async fn send(&mut self, message: Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap();
    }

    /// A request's whole response (`result` or `error`), skipping notifications.
    pub async fn exchange(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        loop {
            let line =
                tokio::time::timeout(std::time::Duration::from_secs(20), self.lines.next_line())
                    .await
                    .expect("no answer within 20 s")
                    .unwrap()
                    .expect("the server closed the pipe");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    /// A request's `result`.
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let answer = self.exchange(method, params).await;
        assert!(answer.get("error").is_none(), "{method} failed: {answer}");
        answer["result"].clone()
    }

    pub async fn tools(&mut self) -> Vec<Value> {
        self.request("tools/list", json!({})).await["tools"]
            .as_array()
            .unwrap()
            .clone()
    }

    pub async fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
            .await
    }
}

type Pipe = Rpc<
    tokio::io::ReadHalf<tokio::io::DuplexStream>,
    tokio::io::WriteHalf<tokio::io::DuplexStream>,
>;

async fn connect_to(server: GithubServer) -> Pipe {
    let (client, served) = tokio::io::duplex(1 << 20);
    let (sr, sw) = tokio::io::split(served);
    tokio::spawn(async move {
        if let Ok(running) = server.serve((sr, sw)).await {
            let _ = running.waiting().await;
        }
    });
    let (cr, cw) = tokio::io::split(client);
    Rpc::connect(cr, cw).await
}

async fn connect(world: &World) -> Pipe {
    connect_to(GithubServer::new(Arc::new(world.ctx.clone()))).await
}

/// The one text block of a result.
#[track_caller]
fn text(result: &Value) -> String {
    let content = result["content"].as_array().expect("content");
    assert_eq!(content.len(), 1, "expected exactly one content block");
    assert_eq!(content[0]["type"], "text");
    content[0]["text"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn the_server_identifies_itself_offers_tools_only_and_tells_a_client_how_to_use_it() {
    let world = without_token(vec![]);
    let client = connect(&world).await;
    let init = &client.initialized;
    assert_eq!(init["serverInfo"]["name"], SERVER_NAME);
    assert_eq!(init["serverInfo"]["version"], VERSION);
    let keys: Vec<_> = init["capabilities"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, vec!["tools".to_string()]);
    let instructions = init["instructions"].as_str().unwrap_or("");
    assert_matches(instructions, "GITHUB_API_KEY");
    assert_matches(instructions, "'owner/name'");
}

#[tokio::test]
async fn tools_list_advertises_exactly_the_surface_with_schemas_and_hints() {
    let world = without_token(vec![]);
    let mut client = connect(&world).await;
    let tools = client.tools().await;
    let mut names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, tool_names());
    for tool in &tools {
        let name = tool["name"].as_str().unwrap();
        assert_eq!(
            tool["outputSchema"]["type"], "object",
            "{name} has no output schema"
        );
        assert!(tool["description"].as_str().unwrap_or("").chars().count() > 20);
        let (read_only, destructive, idempotent, open_world) = kind_hints(kind_of(name).unwrap());
        let a = &tool["annotations"];
        assert_eq!(
            (
                a["readOnlyHint"].as_bool(),
                a["destructiveHint"].as_bool(),
                a["idempotentHint"].as_bool(),
                a["openWorldHint"].as_bool()
            ),
            (
                Some(read_only),
                Some(destructive),
                Some(idempotent),
                Some(open_world)
            ),
            "{name}"
        );
    }
    let create = tools.iter().find(|t| t["name"] == "repo_create").unwrap();
    assert_eq!(create["inputSchema"]["required"], json!(["name"]));
}

#[tokio::test]
async fn repo_create_over_the_protocol_puts_the_repository_in_the_text_and_structured_content() {
    let world = with_token(vec![reply(
        201,
        json!({
            "name": "demo",
            "full_name": "acme/demo",
            "html_url": "https://github.com/acme/demo",
            "clone_url": "https://github.com/acme/demo.git",
            "private": true,
            "visibility": "private",
            "default_branch": "main",
        }),
    )]);
    let mut client = connect(&world).await;
    let result = client
        .call("repo_create", json!({"name": "demo", "org": "acme"}))
        .await;
    assert_eq!(result["isError"], false);
    assert_eq!(
        text(&result),
        "**Created private repository acme/demo on GitHub.**\n\n- name: demo\n- full_name: acme/demo\n- html_url: https://github.com/acme/demo\n- clone_url: https://github.com/acme/demo.git\n- visibility: private\n- default_branch: main"
    );
    assert_eq!(
        result["structuredContent"]["html_url"],
        "https://github.com/acme/demo"
    );
    assert!(result["structuredContent"].get("summary").is_none());
}

#[tokio::test]
async fn a_refusal_is_an_error_result_with_the_reason_and_no_structured_content() {
    let world = without_token(vec![]);
    let mut client = connect(&world).await;
    let result = client.call("repo_create", json!({"name": "demo"})).await;
    assert_eq!(result["isError"], true);
    assert!(result.get("structuredContent").is_none());
    assert_matches(
        &text(&result),
        r"^\*\*Refused\*\*\n\nGITHUB_API_KEY is not set\.",
    );
}

#[tokio::test]
async fn a_call_with_a_missing_argument_or_to_no_such_tool_is_an_error_result_and_not_a_crash() {
    let world = without_token(vec![]);
    let mut client = connect(&world).await;
    let missing = client.call("repo_create", json!({})).await;
    assert_eq!(missing["isError"], true);
    assert_matches(&text(&missing), "name");

    let unknown = client.call("no_such_tool", json!({})).await;
    assert_eq!(unknown["isError"], true);
    assert_matches(&text(&unknown), "(?i)not found");

    assert_eq!(client.tools().await.len(), tool_names().len());
}

fn crashes<'a>(_: &'a Ctx, _: &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>> {
    Box::pin(async { panic!("boom") })
}

fn crashes_with_token<'a>(_: &'a Ctx, _: &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>> {
    Box::pin(async { panic!("upstream said {TOKEN}") })
}

/// The registered tools, with `repo_create`'s handler swapped for `handle`.
fn tools_with(handle: lxagents_github::models::tool::Handler) -> Vec<ToolSpec> {
    controllers::tools()
        .iter()
        .cloned()
        .map(|mut spec| {
            if spec.name == "repo_create" {
                spec.handle = handle;
            }
            spec
        })
        .collect()
}

#[tokio::test]
async fn a_handler_that_crashes_is_reported_as_an_internal_error_and_the_server_carries_on() {
    let world = without_token(vec![]);
    let ctx = Arc::new(world.ctx.clone());
    let mut client = connect_to(GithubServer::with_tools(ctx, tools_with(crashes))).await;
    let result = client.call("repo_create", json!({"name": "demo"})).await;
    assert_eq!(result["isError"], true);
    assert!(result.get("structuredContent").is_none());
    // Rust has no exception classes: the TypeScript "TypeError: boom" is "Error: boom".
    assert_eq!(text(&result), "**Internal Error**\n\nError: boom");
    assert_matches(&world.logs()[0], r"^Handler crashed:\nError: boom");

    // The same server keeps serving, and the real handler on a fresh server refuses as usual.
    let again = client.call("repo_create", json!({"name": "demo"})).await;
    assert_eq!(again["isError"], true);
    let mut normal = connect(&world).await;
    let after = normal.call("repo_create", json!({"name": "demo"})).await;
    assert_matches(&text(&after), r"^\*\*Refused\*\*");
}

#[tokio::test]
async fn a_crash_never_carries_the_token_in_the_result_or_in_the_log() {
    let world = with_token(vec![]);
    let ctx = Arc::new(world.ctx.clone());
    let mut client = connect_to(GithubServer::with_tools(
        ctx,
        tools_with(crashes_with_token),
    ))
    .await;
    let result = client.call("repo_create", json!({"name": "demo"})).await;
    assert_eq!(result["isError"], true);
    assert_eq!(
        text(&result),
        "**Internal Error**\n\nError: upstream said ***"
    );
    let logs = world.logs();
    assert!(logs.iter().all(|line| !line.contains(TOKEN)));
    assert_matches(&logs[0], r"upstream said \*\*\*");
}

#[test]
fn tool_result_renders_a_success_and_a_refusal() {
    let as_json = |r| serde_json::to_value(r).unwrap();
    let ok = as_json(tool_result(
        "Done.",
        obj(json!({"id": 7, "text": "body"})),
        true,
        "",
    ));
    assert_eq!(ok["isError"], false);
    assert_eq!(text(&ok), "**Done.**\n\n- id: 7\n\n```text\nbody\n```");
    assert_eq!(ok["structuredContent"], json!({"id": 7, "text": "body"}));

    let refused = as_json(tool_result("Boom", Map::new(), false, "why"));
    assert_eq!(text(&refused), "**Boom**\n\nwhy");
    assert!(refused.get("structuredContent").is_none());

    assert_eq!(
        text(&as_json(tool_result("", Map::new(), false, "why"))),
        "**Refused**\n\nwhy"
    );
}
