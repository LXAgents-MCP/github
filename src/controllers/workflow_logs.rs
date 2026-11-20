//! Read the logs of the jobs of a GitHub Actions workflow run.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::{MAX_LOG_CHARS, MAX_LOG_JOBS, MAX_PER_PAGE};
use crate::models::address::{encode_component, parse_repo};
use crate::models::args::{Args, Range, arg, optional_flag, optional_int, required_int};
use crate::models::body::{Fields, get, list, s, truthy};
use crate::models::shapes::job_fields;
use crate::utils::text::{clean_log, tail};

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_logs",
        description: "Read the logs of the jobs of a GitHub Actions workflow run. By default only the failed jobs, up to 5, each cut to its last lines, where a failure is explained; job_id reads one job and only_failed false reads every job. At most 20,000 characters in all. Find run and job ids with workflow_status.",
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
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Workflow run id."
    },
    "job_id": {
      "description": "Read just this job of the run. Default: the failed jobs.",
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991
    },
    "only_failed": {
      "description": "false reads every job of the run, not only the failed ones. Default true. Ignored with job_id.",
      "type": "boolean"
    }
  },
  "required": [
    "repo",
    "run_id"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "run_id": {
      "type": "number"
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
          },
          "log": {
            "type": "string"
          },
          "truncated": {
            "type": "boolean"
          },
          "note": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "name",
          "status",
          "truncated"
        ],
        "additionalProperties": false
      }
    },
    "truncated": {
      "type": "boolean"
    }
  },
  "required": [
    "run_id",
    "job_count",
    "jobs",
    "truncated"
  ],
  "additionalProperties": false
}"#;

fn is_failed(job: &Value) -> bool {
    matches!(
        get(job, "conclusion").as_str(),
        Some("failure") | Some("timed_out")
    )
}

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let run_id = required_int(arg(args, "run_id"), "run_id", Range::min(1))?;
    let job_id = optional_int(arg(args, "job_id"), "job_id", Range::min(1))?;
    let only_failed = optional_flag(arg(args, "only_failed"), "only_failed", true)?;

    let explain = |status: u16, detail: &str| {
        (status == 404).then(|| {
            format!(
                "GitHub could not find workflow run {run_id} in {} (404){detail}. Find run ids with workflow_status. GitHub also answers 404 for a private repository the token cannot see.",
                repo.full
            )
        })
    };
    let listing = github(
        ctx,
        "GET",
        &format!("{}/actions/runs/{run_id}/jobs", repo.path),
        GitHubOptions::what(format!(
            "list the jobs of workflow run {run_id} of {}",
            repo.full
        ))
        .query("per_page", MAX_PER_PAGE)
        .explain(&explain),
    )
    .await?
    .body;
    let all = list(get(&listing, "jobs"));

    let selected: Vec<&Value> = match job_id {
        Some(job_id) => {
            let selected: Vec<&Value> = all
                .iter()
                .filter(|job| get(job, "id").as_f64() == Some(job_id as f64))
                .collect();
            if selected.is_empty() {
                return Err(Refused(format!(
                    "Job {job_id} is not among the {} jobs listed for run {run_id} in {}. Read the run with workflow_status to see its jobs.",
                    all.len(),
                    repo.full
                )));
            }
            selected
        }
        None if only_failed => all.iter().filter(|job| is_failed(job)).collect(),
        None => all.iter().collect(),
    };

    if selected.is_empty() {
        let unfinished = all
            .iter()
            .any(|job| get(job, "status").as_str() != Some("completed"));
        let summary = if all.is_empty() {
            format!("Run {run_id} in {} has no jobs yet.", repo.full)
        } else {
            let so_far = if unfinished {
                " so far, and some jobs have not finished"
            } else {
                ""
            };
            format!(
                "Run {run_id} in {} has no failed jobs{so_far}. Pass only_failed false to read every job, or job_id for one.",
                repo.full
            )
        };
        return Outcome::ok(
            summary,
            Fields::new()
                .set("run_id", run_id)
                .set("job_count", 0)
                .set("jobs", Vec::<Value>::new())
                .set("truncated", false),
        );
    }

    // The jobs share the budget, one request each and one after another, so a matrix of failures is not a
    // burst against the rate limit.
    let reading = &selected[..selected.len().min(MAX_LOG_JOBS)];
    let share = MAX_LOG_CHARS / reading.len();
    let mut jobs: Vec<Value> = Vec::new();
    let mut cut = selected.len() > reading.len();

    for job in reading {
        let fields = Fields(job_fields(job));
        let id = s(get(job, "id"));
        let answer = github(
            ctx,
            "GET",
            &format!("{}/actions/jobs/{}/logs", repo.path, encode_component(&id)),
            GitHubOptions::what(format!("read the log of job {id} of {}", repo.full))
                .raw()
                .allow(&[404, 410]),
        )
        .await?;

        if answer.status == 404 || answer.status == 410 {
            jobs.push(
                fields
                    .set("truncated", false)
                    .set("note", "GitHub has no log for this job: it may not have started, or the log has expired.")
                    .into_value(),
            );
            continue;
        }
        let text = clean_log(answer.body.as_str().unwrap_or(""))
            .trim_end()
            .to_string();
        if text.is_empty() {
            jobs.push(
                fields
                    .set("truncated", false)
                    .set("note", "The log is empty.")
                    .into_value(),
            );
            continue;
        }
        let (log, shortened) = tail(&text, share);
        cut = cut || shortened;
        jobs.push(
            fields
                .set("log", log)
                .set("truncated", shortened)
                .into_value(),
        );
    }

    let mut summary = match job_id {
        Some(job_id) => format!(
            "Log of job '{}' ({job_id}) in run {run_id} of {}.",
            s(get(&jobs[0], "name")),
            repo.full
        ),
        None => {
            let failed = if only_failed { "failed " } else { "" };
            let noun = if jobs.len() == 1 { "job" } else { "jobs" };
            let more = if selected.len() > reading.len() {
                format!(
                    " Showing the first {} of {}; pass job_id for the others.",
                    reading.len(),
                    selected.len()
                )
            } else {
                String::new()
            };
            format!(
                "Logs of {} {failed}{noun} in run {run_id} of {}.{more}",
                jobs.len(),
                repo.full
            )
        }
    };
    if jobs.iter().any(|job| truthy(get(job, "truncated"))) {
        summary = format!("{summary} A long log is cut to its last {share} characters.");
    }
    Outcome::ok(
        summary,
        Fields::new()
            .set("run_id", run_id)
            .set("job_count", selected.len())
            .set("jobs", jobs)
            .set("truncated", cut),
    )
}

handler!(run);
