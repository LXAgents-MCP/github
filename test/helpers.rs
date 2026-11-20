//! Shared test support: a mock network, a mock environment, and the output check.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use lxagents_github::config::config::TOKEN_ENV;
use lxagents_github::controllers;
use lxagents_github::models::tool::ToolDef;
use lxagents_github::services::context::{
    ApiRequest, Backend, Ctx, Env, LogSink, RawResponse, TransportError,
};
use lxagents_github::utils::errors::Refused;
use serde_json::{Map, Value, json};

pub const TOKEN: &str = "ghp_test_token_1234";

/// A JSON object from a `json!` literal.
pub fn obj(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

/// Variables for one test.
#[derive(Default)]
pub struct TestEnv(Mutex<HashMap<String, String>>);

impl TestEnv {
    pub fn set(&self, name: &str, value: &str) {
        self.0
            .lock()
            .unwrap()
            .insert(name.to_string(), value.to_string());
    }
    pub fn remove(&self, name: &str) {
        self.0.lock().unwrap().remove(name);
    }
}

impl Env for TestEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }
}

/// One canned answer: a status, a body, and headers.
#[derive(Clone, Debug)]
pub enum Answer {
    Reply {
        status: u16,
        body: Option<Value>,
        headers: Vec<(String, String)>,
    },
    /// The request never got an answer.
    Fail(TransportError),
}

/// An answer. A string body is sent as it is, anything else as JSON, `Value::Null` as no body.
pub fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply {
        status,
        body: if body.is_null() { None } else { Some(body) },
        headers: vec![],
    }
}

/// An answer with headers.
pub fn reply_with(status: u16, body: Value, headers: &[(&str, &str)]) -> Answer {
    Answer::Reply {
        status,
        body: if body.is_null() { None } else { Some(body) },
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/// One request the mock saw. `body` is the JSON it carried, decoded, or `Value::Null`.
#[derive(Clone, Debug)]
pub struct Call {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
    pub raw_body: Option<String>,
}

impl Call {
    /// A header by name, in any case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A queue of answers, one per request in order, recording every request. A request beyond the
/// queue panics, so an unexpected call cannot go unnoticed.
#[derive(Default)]
pub struct MockApi {
    queue: Mutex<VecDeque<Answer>>,
    calls: Mutex<Vec<Call>>,
}

impl MockApi {
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    pub fn push(&self, answer: Answer) {
        self.queue.lock().unwrap().push_back(answer);
    }
}

impl Backend for MockApi {
    fn send(&self, request: ApiRequest) -> BoxFuture<'_, Result<RawResponse, TransportError>> {
        let body = match &request.body {
            None => Value::Null,
            Some(text) => serde_json::from_str(text).unwrap_or(Value::String(text.clone())),
        };
        let number = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(Call {
                url: request.url.clone(),
                method: request.method.clone(),
                headers: request.headers.clone(),
                body,
                raw_body: request.body.clone(),
            });
            calls.len()
        };
        let next = self.queue.lock().unwrap().pop_front();
        Box::pin(async move {
            match next {
                None => panic!(
                    "unexpected request #{number}: {} {}",
                    request.method, request.url
                ),
                Some(Answer::Fail(error)) => Err(error),
                Some(Answer::Reply {
                    status,
                    body,
                    headers,
                }) => {
                    let bytes = match body {
                        None => Vec::new(),
                        Some(Value::String(text)) => text.into_bytes(),
                        Some(other) => other.to_string().into_bytes(),
                    };
                    let mut map = BTreeMap::new();
                    for (k, v) in headers {
                        map.insert(k.to_lowercase(), v);
                    }
                    Ok(RawResponse {
                        status,
                        headers: map,
                        body: bytes,
                    })
                }
            }
        })
    }
}

/// A test's world: its context, the mock network, the variables, and the log lines written.
pub struct World {
    pub ctx: Ctx,
    pub api: Arc<MockApi>,
    pub env: Arc<TestEnv>,
    pub logs: Arc<Mutex<Vec<String>>>,
}

impl World {
    pub fn calls(&self) -> Vec<Call> {
        self.api.calls()
    }
    pub fn logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }
}

/// A world with the token set and these answers queued.
pub fn with_token(answers: Vec<Answer>) -> World {
    let world = without_token(answers);
    world.env.set(TOKEN_ENV, TOKEN);
    world
}

/// A world with no token and these answers queued.
pub fn without_token(answers: Vec<Answer>) -> World {
    let api = Arc::new(MockApi::default());
    for answer in answers {
        api.push(answer);
    }
    let env = Arc::new(TestEnv::default());
    let logs = Arc::new(Mutex::new(Vec::new()));
    let sink_logs = logs.clone();
    let log: LogSink = Arc::new(move |line: &str| sink_logs.lock().unwrap().push(line.to_string()));
    let ctx = Ctx {
        env: env.clone(),
        http: api.clone(),
        log,
    };
    World {
        ctx,
        api,
        env,
        logs,
    }
}

/// What a handler answered: the payload (`summary` first, then the fields) and whether it succeeded.
pub struct Done {
    pub payload: Value,
    pub ok: bool,
}

/// Run one tool's handler directly, as the server would, with these arguments.
pub async fn call(world: &World, tool: &str, args: Value) -> Result<Done, Refused> {
    let spec = controllers::get(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    let args = obj(args);
    let outcome = (spec.handle)(&world.ctx, &args).await?;
    let mut payload = Map::new();
    payload.insert("summary".into(), Value::String(outcome.summary));
    payload.extend(outcome.fields);
    Ok(Done {
        payload: Value::Object(payload),
        ok: outcome.ok,
    })
}

/// Run a tool that must succeed; check its payload against its output schema; return the payload.
pub async fn ok(world: &World, tool: &str, args: Value) -> Value {
    let done = call(world, tool, args)
        .await
        .unwrap_or_else(|e| panic!("{tool} refused: {}", e.message()));
    expect_output(&def(tool), &done.payload);
    done.payload
}

/// Run a tool that must refuse; return the refusal's message.
pub async fn refused(world: &World, tool: &str, args: Value) -> String {
    match call(world, tool, args).await {
        Ok(done) => panic!("{tool} did not refuse: {}", done.payload),
        Err(e) => e.message().to_string(),
    }
}

/// Assert a refusal message matches a regular expression.
#[track_caller]
pub fn assert_matches(text: &str, pattern: &str) {
    let re = regex::Regex::new(pattern).unwrap();
    assert!(re.is_match(text), "{text:?} does not match /{pattern}/");
}

/// Assert a text does not match a regular expression.
#[track_caller]
pub fn assert_no_match(text: &str, pattern: &str) {
    let re = regex::Regex::new(pattern).unwrap();
    assert!(!re.is_match(text), "{text:?} matches /{pattern}/");
}

/// A tool's definition.
pub fn def(tool: &str) -> ToolDef {
    controllers::get(tool)
        .unwrap_or_else(|| panic!("no tool {tool}"))
        .def
        .clone()
}

/// The input schema's property names, in order.
pub fn input_keys(tool: &str) -> Vec<String> {
    def(tool).input_schema["properties"]
        .as_object()
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default()
}

/// A successful payload must satisfy the tool's own output schema - the check a client makes on
/// `structuredContent` - and lead with a summary sentence.
#[track_caller]
pub fn expect_output(def: &ToolDef, payload: &Value) {
    let map = payload.as_object().expect("payload is an object");
    let summary = map
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} has no summary", def.name));
    assert!(
        !summary.is_empty() && summary == summary.trim(),
        "{} summary is blank or padded",
        def.name
    );
    let mut fields = map.clone();
    fields.remove("summary");
    if let Err(problem) = validate(&def.output_schema, &Value::Object(fields), "") {
        panic!("{} payload breaks its outputSchema: {problem}", def.name);
    }
}

/// The subset of JSON Schema the output schemas use: type, properties, required,
/// additionalProperties: false, items.
pub fn validate(schema: &Value, value: &Value, at: &str) -> Result<(), String> {
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        let fits = match kind {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            other => return Err(format!("{at}: unknown type {other}")),
        };
        if !fits {
            return Err(format!("{at}: expected {kind}, got {value}"));
        }
    }
    if let (Some(props), Some(map)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        for name in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = name.as_str().unwrap();
            if !map.contains_key(name) {
                return Err(format!("{at}.{name}: required"));
            }
        }
        for (key, item) in map {
            match props.get(key) {
                Some(sub) => validate(sub, item, &format!("{at}.{key}"))?,
                None if schema.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    return Err(format!("{at}.{key}: not in the schema"));
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(list)) = (schema.get("items"), value.as_array()) {
        for (i, item) in list.iter().enumerate() {
            validate(items, item, &format!("{at}[{i}]"))?;
        }
    }
    Ok(())
}

/// `json!` shortcut for an empty body.
pub fn none() -> Value {
    json!(null)
}
