//! Read the state of a GitHub Actions workflow run and its jobs.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::MAX_PER_PAGE;
use crate::models::address::{encode_component, parse_repo, ref_name, workflow_name};
use crate::models::args::{Args, Range, arg, optional_int, optional_text};
use crate::models::body::{Fields, at, get, is_int, list, present, s, text};
use crate::models::shapes::{expect_body, job_fields};
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_status",
        description: "Read a GitHub Actions workflow run: its status and conclusion, branch, commit, and each job's result with the step that failed. Give a run_id, or a workflow (optionally on a branch) to read its latest run. After workflow_run, ask again until the status is 'completed'.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: READ,
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
    "run_id": {
      "description": "Run id, from workflow_run or an earlier workflow_status.",
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991
    },
    "workflow": {
      "description": "Workflow id or file name, e.g. 'ci.yml'; reads its most recent run.",
      "type": "string"
    },
    "branch": {
      "description": "With `workflow`: only runs on this branch.",
      "type": "string"
    }
  },
  "required": [
    "repo"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "found": {
      "type": "boolean"
    },
    "run_id": {
      "type": "number"
    },
    "name": {
      "type": "string"
    },
    "status": {
      "type": "string"
    },
    "conclusion": {
      "type": "string"
    },
    "branch": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "event": {
      "type": "string"
    },
    "run_number": {
      "type": "number"
    },
    "attempt": {
      "type": "number"
    },
    "actor": {
      "type": "string"
    },
    "created_at": {
      "type": "string"
    },
    "updated_at": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    },
    "job_count": {
      "type": "number"
    },
    "jobs": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": {
            "type": "number"
          },
          "name": {
            "type": "string"
          },
          "status": {
            "type": "string"
          },
          "conclusion": {
            "type": "string"
          },
          "failed_step": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "name",
          "status"
        ],
        "additionalProperties": false
      }
    }
  },
  "required": [
    "found",
    "jobs"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let run_id = optional_int(arg(args, "run_id"), "run_id", Range::min(1))?;
    let workflow = match optional_text(arg(args, "workflow"), "workflow")? {
        None => None,
        Some(_) => Some(workflow_name(arg(args, "workflow"), "workflow")?),
    };
    let branch = match optional_text(arg(args, "branch"), "branch")? {
        None => None,
        Some(_) => Some(ref_name(arg(args, "branch"), "branch")?),
    };

    if run_id.is_some() && workflow.is_some() {
        return Err(Refused(
            "Give either `run_id` or `workflow`, not both.".to_string(),
        ));
    }
    if run_id.is_none() && workflow.is_none() {
        return Err(Refused(
            "Give `run_id`, or `workflow` to read its latest run.".to_string(),
        ));
    }
    if branch.is_some() && workflow.is_none() {
        return Err(Refused(
            "`branch` only narrows the search for a workflow's latest run; give `workflow` with it.".to_string(),
        ));
    }

    let run: Value = if let Some(run_id) = run_id {
        let explain = |status: u16, detail: &str| {
            (status == 404).then(|| {
                format!(
                    "GitHub could not find workflow run {run_id} in {} (404){detail}. Give a workflow to read its latest run and find the id. GitHub also answers 404 for a private repository the token cannot see.",
                    repo.full
                )
            })
        };
        let answer = github(
            ctx,
            "GET",
            &format!("{}/actions/runs/{run_id}", repo.path),
            GitHubOptions::what(format!("read workflow run {run_id} of {}", repo.full))
                .explain(&explain),
        )
        .await?;
        expect_body(&answer.body, "html_url", "a workflow run")?;
        answer.body
    } else {
        // No run id, so the check above means a workflow was given.
        let named = workflow
            .as_ref()
            .expect("a workflow, since there is no run id");
        let explain = |status: u16, detail: &str| {
            (status == 404).then(|| {
                format!(
                    "GitHub could not find the workflow '{}' in {} (404){detail}. Check it with workflow_list. GitHub also answers 404 for a private repository the token cannot see.",
                    named.name, repo.full
                )
            })
        };
        let answer = github(
            ctx,
            "GET",
            &format!("{}/actions/workflows/{}/runs", repo.path, named.encoded),
            GitHubOptions::what(format!(
                "find the latest run of the workflow '{}' of {}",
                named.name, repo.full
            ))
            .query("per_page", 1)
            .query("branch", branch.clone())
            .explain(&explain),
        )
        .await?;
        let Some(first) = list(get(&answer.body, "workflow_runs")).first().cloned() else {
            let on = branch
                .as_ref()
                .map(|b| format!(" on {b}"))
                .unwrap_or_default();
            return Outcome::ok(
                format!(
                    "No run of workflow '{}'{on} in {} yet. A run appears a few seconds after it is triggered; ask again shortly.",
                    named.name, repo.full
                ),
                Fields::new()
                    .set("found", false)
                    .set("jobs", Vec::<Value>::new()),
            );
        };
        expect_body(&first, "html_url", "a workflow run")?;
        first
    };

    let id = s(get(&run, "id"));
    let listing = github(
        ctx,
        "GET",
        &format!("{}/actions/runs/{}/jobs", repo.path, encode_component(&id)),
        GitHubOptions::what(format!(
            "list the jobs of workflow run {id} of {}",
            repo.full
        ))
        .query("per_page", MAX_PER_PAGE),
    )
    .await?
    .body;
    let jobs: Vec<Value> = list(get(&listing, "jobs"))
        .iter()
        .map(|job| Value::Object(job_fields(job)))
        .collect();
    let failed: Vec<String> = jobs
        .iter()
        .filter(|job| {
            matches!(
                get(job, "conclusion").as_str(),
                Some("failure") | Some("timed_out")
            )
        })
        .map(|job| s(get(job, "name")))
        .collect();

    let name = {
        let name = get(&run, "name");
        let name = if name.is_null() {
            get(&run, "display_title")
        } else {
            name
        };
        if name.is_null() {
            Value::from("")
        } else {
            name.clone()
        }
    };
    let named = if crate::models::body::truthy(&name) {
        format!(" ({})", s(&name))
    } else {
        String::new()
    };
    let place = text(get(&run, "head_branch"))
        .map(|b| format!(" on {b}"))
        .unwrap_or_default();
    let state = if get(&run, "status").as_str() == Some("completed") {
        let conclusion = get(&run, "conclusion");
        format!(
            "completed, {}",
            if conclusion.is_null() {
                "with no conclusion".to_string()
            } else {
                s(conclusion)
            }
        )
    } else {
        s(get(&run, "status"))
    };
    let failures = if failed.is_empty() {
        String::new()
    } else {
        format!(" Failed: {}.", failed.join(", "))
    };
    let int = |key: &str| is_int(get(&run, key)).then(|| get(&run, key).clone());
    let job_count = if is_int(get(&listing, "total_count")) {
        get(&listing, "total_count").clone()
    } else {
        jobs.len().into()
    };

    Outcome::ok(
        format!(
            "Workflow run {id}{named}{place} at {} in {} is {state}.{failures}",
            short(get(&run, "head_sha")),
            repo.full
        ),
        Fields::new()
            .set("found", true)
            .set("run_id", get(&run, "id").clone())
            .opt("name", present(&name))
            .opt("status", present(get(&run, "status")))
            .opt("conclusion", present(get(&run, "conclusion")))
            .opt("branch", present(get(&run, "head_branch")))
            .opt("sha", present(get(&run, "head_sha")))
            .opt("event", present(get(&run, "event")))
            .opt("run_number", int("run_number"))
            .opt("attempt", int("run_attempt"))
            .opt("actor", text(at(&run, "actor.login")))
            .opt("created_at", present(get(&run, "created_at")))
            .opt("updated_at", present(get(&run, "updated_at")))
            .set("html_url", get(&run, "html_url").clone())
            .set("job_count", job_count)
            .set("jobs", jobs),
    )
}

handler!(run);
