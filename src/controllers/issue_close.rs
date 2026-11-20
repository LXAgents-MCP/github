//! Close an issue.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::json;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, one_of_or, raw_text, required_int};
use crate::models::body::{Fields, get, text};
use crate::models::shapes::expect_body;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_close",
        description: "Close an issue of a GitHub repository, as completed, not planned or a duplicate, optionally leaving a comment first. It only closes: there is no tool to reopen. Use pr_close for a pull request.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: UPDATE,
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Issue number."
    },
    "reason": {
      "description": "completed (default), not_planned or duplicate.",
      "type": "string",
      "enum": [
        "completed",
        "not_planned",
        "duplicate"
      ]
    },
    "comment": {
      "description": "A comment to post before closing, e.g. why.",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "number"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "state": {
      "type": "string"
    },
    "state_reason": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "number",
    "state",
    "html_url"
  ],
  "additionalProperties": false
}"#;

const REASONS: &[&str] = &["completed", "not_planned", "duplicate"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let reason = one_of_or(arg(args, "reason"), "reason", REASONS, "completed")?;
    let comment =
        raw_text(arg(args, "comment"), "comment")?.filter(|comment| !comment.trim().is_empty());

    if let Some(comment) = &comment {
        github(
            ctx,
            "POST",
            &format!("{}/issues/{number}/comments", repo.path),
            GitHubOptions::what(format!(
                "comment on issue #{number} of {} before closing it",
                repo.full
            ))
            .json(json!({"body": comment})),
        )
        .await?;
    }

    let closed = github(
        ctx,
        "PATCH",
        &format!("{}/issues/{number}", repo.path),
        GitHubOptions::what(format!("close issue #{number} of {}", repo.full))
            .json(json!({"state": "closed", "state_reason": reason})),
    )
    .await
    .map_err(|refused| match &comment {
        Some(_) => Refused(format!(
            "{} The comment had already been posted.",
            refused.message()
        )),
        None => refused,
    })?;

    let body = expect_body(&closed.body, "html_url", "an issue")?;
    Outcome::ok(
        format!(
            "Closed issue #{number} in {} as {}.",
            repo.full,
            reason.replacen('_', " ", 1)
        ),
        Fields::new()
            .set("number", number)
            .set("state", get(body, "state").clone())
            .opt("state_reason", text(get(body, "state_reason")))
            .set("html_url", get(body, "html_url").clone()),
    )
}

handler!(run);
