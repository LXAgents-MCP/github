//! Start a GitHub Actions workflow by dispatch.
use crate::handler;
use crate::models::schemas::{WRITE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::{Map, Value};

use crate::config::config::MAX_DISPATCH_INPUTS;
use crate::models::address::{optional_ref, parse_repo, workflow_name};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, get, is_int, s};

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_run",
        description: "Start a GitHub Actions workflow run on a branch or tag (default: the default branch), with optional inputs. It runs whatever the workflow does, which may deploy or publish. The workflow needs an 'on: workflow_dispatch' trigger and its file must be on the default branch. Returns the run id when GitHub gives it; follow the run with workflow_status.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: WRITE,
    }
}

const INPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "repo": {
      "type": "string",
      "description": "Repository as 'owner/name', for example 'LXAgents-MCP/github'."
    },
    "workflow": {
      "type": "string",
      "description": "Workflow id or file name, e.g. 'ci.yml'. See workflow_list."
    },
    "ref": {
      "description": "Branch or tag to run it on. Default: the repository's default branch.",
      "type": "string"
    },
    "inputs": {
      "description": "The workflow's inputs by name, up to 25. Values are sent as text.",
      "type": "object",
      "propertyNames": {
        "type": "string"
      },
      "additionalProperties": {
        "type": [
          "string",
          "number",
          "boolean"
        ]
      }
    }
  },
  "required": [
    "repo",
    "workflow"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "dispatched": {
      "type": "boolean"
    },
    "workflow": {
      "type": "string"
    },
    "ref": {
      "type": "string"
    },
    "run_id": {
      "type": "number"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "dispatched",
    "workflow",
    "ref"
  ],
  "additionalProperties": false
}"#;

/// The inputs of a dispatch, each as the string GitHub wants, or `None` when there are none.
fn dispatch_inputs(value: &Value) -> Result<Option<Map<String, Value>>, Refused> {
    let entries = match value {
        Value::Null => return Ok(None),
        Value::Object(entries) => entries,
        _ => {
            return Err(Refused(
                "`inputs` must be an object of input names and their values.".to_string(),
            ));
        }
    };
    if entries.len() > MAX_DISPATCH_INPUTS {
        return Err(Refused(format!(
            "`inputs` may hold at most {MAX_DISPATCH_INPUTS} inputs, got {}.",
            entries.len()
        )));
    }
    let mut inputs = Map::new();
    for (key, entry) in entries {
        if key.trim().is_empty() {
            return Err(Refused(
                "`inputs` has an input with a blank name.".to_string(),
            ));
        }
        if !matches!(entry, Value::String(_) | Value::Number(_) | Value::Bool(_)) {
            return Err(Refused(format!(
                "`inputs.{key}` must be a string, a number or true/false."
            )));
        }
        inputs.insert(key.clone(), Value::String(s(entry)));
    }
    Ok(if entries.is_empty() {
        None
    } else {
        Some(inputs)
    })
}

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let workflow = workflow_name(arg(args, "workflow"), "workflow")?;
    let given = optional_ref(arg(args, "ref"), "ref")?;
    let inputs = dispatch_inputs(arg(args, "inputs"))?;

    let reference = match given {
        Some(reference) => reference,
        None => {
            let answer = github(
                ctx,
                "GET",
                &repo.path,
                GitHubOptions::what(format!("find the default branch of {}", repo.full)),
            )
            .await?;
            match get(&answer.body, "default_branch").as_str() {
                Some(reference) => reference.to_string(),
                None => {
                    return Err(Refused(format!(
                        "Could not find the default branch of {}; pass `ref`.",
                        repo.full
                    )));
                }
            }
        }
    };

    let explain = |status: u16, detail: &str| match status {
        404 => Some(format!(
            "GitHub could not find the workflow '{}' in {} (404){detail}. Check it with workflow_list: the workflow file must exist on the repository's default branch. GitHub also answers 404 for a private repository the token cannot see.",
            workflow.name, repo.full
        )),
        422 => Some(format!(
            "GitHub would not start '{}' on '{reference}' in {} (422){detail}. The workflow needs an 'on: workflow_dispatch' trigger, every input it requires must be given, and no input it does not declare.",
            workflow.name, repo.full
        )),
        _ => None,
    };
    // Asking for the run's details is what makes GitHub say which run it started; without it the answer
    // is empty. An answer with no body is handled below, so the call works either way.
    let json = Fields::new()
        .set("ref", reference.clone())
        .opt("inputs", inputs.map(Value::Object))
        .set("return_run_details", true)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!(
            "{}/actions/workflows/{}/dispatches",
            repo.path, workflow.encoded
        ),
        GitHubOptions::what(format!(
            "run the workflow '{}' of {} on '{reference}'",
            workflow.name, repo.full
        ))
        .json(json)
        .explain(&explain),
    )
    .await?;
    let body = &answer.body;

    let run_id = is_int(get(body, "workflow_run_id")).then(|| get(body, "workflow_run_id").clone());
    let url = get(body, "html_url").as_str().map(str::to_string);
    let summary = match &run_id {
        None => format!(
            "Asked GitHub to run workflow '{}' on {reference} in {}. GitHub did not say which run it started; find it with workflow_status in a few seconds.",
            workflow.name, repo.full
        ),
        Some(id) => format!(
            "Started workflow '{}' on {reference} in {}: run {}.",
            workflow.name,
            repo.full,
            s(id)
        ),
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("dispatched", true)
            .set("workflow", workflow.name.clone())
            .set("ref", reference.clone())
            .opt("run_id", run_id)
            .opt("html_url", url),
    )
}

handler!(run);
