//! Close a pull request without merging it.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::json;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, raw_text, required_int};
use crate::models::body::{Fields, boolean, get, truthy};
use crate::models::shapes::expect_body;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_close",
        description: "Close a pull request in a GitHub repository without merging it, optionally leaving a comment first. Its branch is kept. There is no tool to reopen it.",
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
      "description": "Pull request number."
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
    "merged": {
      "type": "boolean"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "number",
    "state",
    "merged",
    "html_url"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let comment =
        raw_text(arg(args, "comment"), "comment")?.filter(|comment| !comment.trim().is_empty());

    if let Some(comment) = &comment {
        github(
            ctx,
            "POST",
            &format!("{}/issues/{number}/comments", repo.path),
            GitHubOptions::what(format!(
                "comment on pull request #{number} of {} before closing it",
                repo.full
            ))
            .json(json!({"body": comment})),
        )
        .await?;
    }

    let closed = github(
        ctx,
        "PATCH",
        &format!("{}/pulls/{number}", repo.path),
        GitHubOptions::what(format!("close pull request #{number} of {}", repo.full))
            .json(json!({"state": "closed"})),
    )
    .await
    .map_err(|refused| match &comment {
        Some(_) => Refused(format!(
            "{} The comment had already been posted.",
            refused.message()
        )),
        None => refused,
    })?;

    let body = expect_body(&closed.body, "html_url", "a pull request")?;
    let summary = if truthy(get(body, "merged")) {
        format!(
            "Pull request #{number} in {} was already merged; nothing to close.",
            repo.full
        )
    } else {
        format!(
            "Closed pull request #{number} in {} without merging.",
            repo.full
        )
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("number", number)
            .set("state", get(body, "state").clone())
            .set("merged", boolean(get(body, "merged")))
            .set("html_url", get(body, "html_url").clone()),
    )
}

handler!(run);
