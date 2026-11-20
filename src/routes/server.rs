//! The MCP server: protocol and tool dispatch only.
//!
//! A fresh instance is created per connection - one for stdio, one per HTTP request. The tool surface is
//! not written here - it is whatever `src/controllers/mod.rs` registers.
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::{Arc, Once};
use std::task::{Context, Poll};

use futures::FutureExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};

use crate::config::version::{SERVER_NAME, VERSION};
use crate::controllers;
use crate::models::args::Args;
use crate::models::tool::{ToolDef, ToolSpec};
use crate::services::auth::token_to_scrub;
use crate::services::context::Ctx;
use crate::utils::redact::redact;
use crate::utils::render::render_payload;

/// What every client reads at `initialize`, before it has called a single tool.
pub const INSTRUCTIONS: &str = "GitHub through its REST API: repositories, branches, files, commits, tags, releases, issues, pull requests and Actions workflows. Every tool needs GITHUB_API_KEY in this server's environment. Name a repository as 'owner/name'. Tools that delete or merge are destructive: confirm with the owner before calling them.";

/// Shape a handler's payload into a `tools/call` result.
///
/// `isError` is how a client learns a call failed. It carries the message, so a failure arrives as
/// readable data rather than as an opaque protocol error. The text block is what the model actually
/// reads (see `render.rs`); everything also rides along in `structuredContent` for a client that would
/// rather read it programmatically - except on a refusal or a crash, which have no fields to report. A
/// client validates `structuredContent` against the tool's output schema whenever it is present, so an
/// empty object there would be rejected for the fields it lacks.
///
/// `message` is the reason, for a refusal or a handler bug; empty for an answer.
pub fn tool_result(
    summary: &str,
    fields: Map<String, Value>,
    ok: bool,
    message: &str,
) -> CallToolResult {
    let text = if message.is_empty() {
        render_payload(summary, &fields)
    } else {
        let summary = if summary.is_empty() {
            "Refused"
        } else {
            summary
        };
        format!("**{summary}**\n\n{message}")
    };
    let mut result = if ok {
        CallToolResult::success(vec![ContentBlock::text(text)])
    } else {
        CallToolResult::error(vec![ContentBlock::text(text)])
    };
    result.is_error = Some(!ok);
    result.structured_content = if message.is_empty() {
        Some(Value::Object(fields))
    } else {
        None
    };
    result
}

thread_local! {
    /// Set while a handler is being polled, so the panic hook leaves the report to `call_tool`.
    static IN_HANDLER: Cell<bool> = const { Cell::new(false) };
    /// Where the last handler panic happened, for the log line.
    static PANIC_AT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Install the panic hook once: a panic inside a handler is reported by `call_tool`, scrubbed of the
/// key, and not printed again by Rust's default hook, which knows nothing of secrets.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if IN_HANDLER.with(Cell::get) {
                let at = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                PANIC_AT.with(|cell| *cell.borrow_mut() = at);
            } else {
                default(info);
            }
        }));
    });
}

/// A handler's future, marked as one for the panic hook on every poll.
struct Marked<F>(Pin<Box<F>>);

impl<F: Future> Future for Marked<F> {
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let before = IN_HANDLER.with(|flag| flag.replace(true));
        let result = self.0.as_mut().poll(cx);
        IN_HANDLER.with(|flag| flag.set(before));
        result
    }
}

/// The text of a panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        text.to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a handler panicked".to_string()
    }
}

/// Run one tool call, turning every failure into a result the client can read.
pub async fn call_tool(ctx: &Ctx, spec: &ToolSpec, args: &Args) -> CallToolResult {
    install_panic_hook();
    let run = AssertUnwindSafe(Marked(Box::pin((spec.handle)(ctx, args))))
        .catch_unwind()
        .await;
    match run {
        Ok(Ok(outcome)) => tool_result(&outcome.summary, outcome.fields, outcome.ok, ""),
        Ok(Err(refused)) => tool_result("Refused", Map::new(), false, refused.message()),
        Err(payload) => {
            // A crash is text nobody wrote for a reader, so it is scrubbed like any other: the token must
            // not reach the log or the client whatever put it into a panic message.
            let secret = token_to_scrub(ctx);
            let message = format!("Error: {}", panic_message(payload.as_ref()));
            let at = PANIC_AT.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
            ctx.log(&redact(
                &format!("Handler crashed:\n{message}\n    at {at}"),
                &secret,
            ));
            tool_result(
                "Internal Error",
                Map::new(),
                false,
                &redact(&message, &secret),
            )
        }
    }
}

/// A tool as `tools/list` publishes it.
pub fn published(def: &ToolDef) -> Tool {
    let object = |value: &Value| Arc::new(value.as_object().cloned().unwrap_or_default());
    let hints = def.annotations;
    Tool::new(
        Cow::Borrowed(def.name),
        Cow::Borrowed(def.description),
        object(&def.input_schema),
    )
    .with_raw_output_schema(object(&def.output_schema))
    .with_annotations(
        ToolAnnotations::new()
            .read_only(hints.read_only)
            .destructive(hints.destructive)
            .idempotent(hints.idempotent)
            .open_world(hints.open_world),
    )
}

/// The server one connection talks to.
#[derive(Clone)]
pub struct GithubServer {
    ctx: Arc<Ctx>,
    tools: Arc<Vec<ToolSpec>>,
}

impl GithubServer {
    /// A server for the registered tools.
    pub fn new(ctx: Arc<Ctx>) -> Self {
        GithubServer {
            ctx,
            tools: Arc::new(controllers::tools().to_vec()),
        }
    }

    /// A server for another set of tools, for a test that needs one to misbehave.
    pub fn with_tools(ctx: Arc<Ctx>, tools: Vec<ToolSpec>) -> Self {
        GithubServer {
            ctx,
            tools: Arc::new(tools),
        }
    }
}

impl ServerHandler for GithubServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(Implementation::new(SERVER_NAME, VERSION))
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.tools.iter().map(|spec| published(&spec.def)).collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(spec) = self.tools.iter().find(|spec| spec.name == request.name) else {
            // Answered as a result, not a protocol error, as the TypeScript SDK did, so the model reads it.
            let message = format!("MCP error -32602: Tool {} not found", request.name);
            let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
            result.is_error = Some(true);
            return Ok(result.into());
        };
        let args = request.arguments.unwrap_or_default();
        Ok(call_tool(&self.ctx, spec, &args).await.into())
    }
}
