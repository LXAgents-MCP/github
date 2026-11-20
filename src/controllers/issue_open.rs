//! Open an issue.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, optional_int, raw_text, required_text, text_list};
use crate::models::body::{Fields, get, s};
use crate::models::schemas::{WRITE, schema};
use crate::models::shapes::{expect_body, names_of};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_open",
        description: "Open a new issue in a GitHub repository, with optional body, labels, assignees and milestone. GitHub ignores labels, assignees and milestone from a token without push access; the result lists what was actually applied.",
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
    "title": {
      "type": "string",
      "description": "Issue title."
    },
    "body": {
      "description": "Issue body, in Markdown.",
      "type": "string"
    },
    "labels": {
      "description": "Label names to apply. They must already exist.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "assignees": {
      "description": "GitHub logins to assign.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "milestone": {
      "description": "Milestone number (not its title).",
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991
    }
  },
  "required": [
    "repo",
    "title"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "title": {
      "type": "string"
    },
    "state": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    },
    "labels": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "assignees": {
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "required": [
    "number",
    "title",
    "state",
    "html_url",
    "labels",
    "assignees"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let title = required_text(arg(args, "title"), "title")?;
    let body = raw_text(arg(args, "body"), "body")?;
    let labels = text_list(arg(args, "labels"), "labels", false, 100)?;
    let assignees = text_list(arg(args, "assignees"), "assignees", false, 100)?;
    let milestone = optional_int(arg(args, "milestone"), "milestone", Range::min(1))?;

    let json = Fields::new()
        .set("title", title)
        .opt("body", body)
        .opt("labels", labels.clone())
        .opt("assignees", assignees.clone())
        .opt("milestone", milestone)
        .into_value();
    let explain = |status: u16, detail: &str| {
        (status == 410).then(|| {
            format!("Issues are disabled in {} (410){detail}. Turn them on with repo_update (has_issues).", repo.full)
        })
    };
    let answer = github(
        ctx,
        "POST",
        &format!("{}/issues", repo.path),
        GitHubOptions::what(format!("open an issue in {}", repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;
    let created = expect_body(&answer.body, "html_url", "an issue")?;

    let applied_labels = names_of(get(created, "labels"));
    let applied_assignees = names_of(get(created, "assignees"));
    let mut dropped: Vec<String> = Vec::new();
    for name in labels.unwrap_or_default() {
        if !applied_labels.contains(&name) {
            dropped.push(format!("label {name}"));
        }
    }
    for login in assignees.unwrap_or_default() {
        if !applied_assignees.contains(&login) {
            dropped.push(format!("assignee {login}"));
        }
    }
    let note = if dropped.is_empty() {
        String::new()
    } else {
        format!(
            " GitHub did not apply: {}. The token may lack push access, or the name does not exist.",
            dropped.join(", ")
        )
    };
    Outcome::ok(
        format!(
            "Opened issue #{} in {}: {}.{note}",
            s(get(created, "number")),
            repo.full,
            s(get(created, "title"))
        ),
        Fields::new()
            .set("number", get(created, "number").clone())
            .set("title", get(created, "title").clone())
            .set("state", get(created, "state").clone())
            .set("html_url", get(created, "html_url").clone())
            .set("labels", applied_labels)
            .set("assignees", applied_assignees),
    )
}

handler!(run);
