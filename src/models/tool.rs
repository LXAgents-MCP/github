//! The contract between a tool module in `src/controllers/` and the server that serves it.
//!
//! A module exports `def()`, a `ToolDef`, and `handle`, a `Handler`. The registry
//! (`src/controllers/mod.rs`) pairs the two into a `ToolSpec`; the protocol layer
//! (`src/routes/server.rs`) only ever sees `ToolSpec`s.
use futures::future::BoxFuture;
use serde_json::{Map, Value};

use super::args::Args;
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

/// The four hints a client reads to decide whether to ask the owner first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hints {
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
}

/// What a client reads about a tool in `tools/list`.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    /// A JSON Schema object, as published.
    pub input_schema: Value,
    pub output_schema: Value,
    pub annotations: Hints,
}

/// A tool's answer: a one-sentence `summary`, fields that satisfy its output schema, and whether it
/// succeeded. A failure is a `Refused`, not an outcome.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub summary: String,
    pub fields: Map<String, Value>,
    pub ok: bool,
}

impl Outcome {
    /// A successful answer.
    pub fn ok(
        summary: impl Into<String>,
        fields: impl Into<Map<String, Value>>,
    ) -> Result<Outcome, Refused> {
        Ok(Outcome {
            summary: summary.into(),
            fields: fields.into(),
            ok: true,
        })
    }
}

impl From<super::body::Fields> for Map<String, Value> {
    fn from(fields: super::body::Fields) -> Self {
        fields.0
    }
}

/// What a tool runs: its arguments as sent, checked by the tool itself.
pub type Handler = for<'a> fn(&'a Ctx, &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>>;

/// One tool as the registry holds it.
#[derive(Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub def: ToolDef,
    pub handle: Handler,
}

/// Give a tool module its `handle`, from an `async fn run(ctx: &Ctx, args: &Args)`.
#[macro_export]
macro_rules! handler {
    ($run:ident) => {
        pub fn handle<'a>(
            ctx: &'a $crate::services::context::Ctx,
            args: &'a $crate::models::args::Args,
        ) -> ::futures::future::BoxFuture<
            'a,
            Result<$crate::models::tool::Outcome, $crate::utils::errors::Refused>,
        > {
            Box::pin($run(ctx, args))
        }
    };
}
