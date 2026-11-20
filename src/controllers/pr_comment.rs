//! Comment on a pull request's conversation.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, required_int, required_raw_text};
use crate::models::body::{Fields, get};
use crate::models::schemas::{WRITE, schema};
use crate::models::shapes::expect_body;
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_comment",
        description: "Add a comment to a pull request's conversation in a GitHub repository. A conversation comment only: it is not attached to a line of the diff and is not a review. Visible to everyone who can see the pull request.",
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Pull request number."
    },
    "body": {
      "type": "string",
      "description": "Comment text, in Markdown."
    }
  },
  "required": [
    "repo",
    "number",
    "body"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "id": {
      "type": "number"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "html_url"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let body = required_raw_text(arg(args, "body"), "body", false)?;

    // GitHub keeps a pull request's conversation comments with its issue comments.
    let explain = |status: u16, detail: &str| {
        (status == 403 && detail.to_lowercase().contains("locked")).then(|| {
            format!(
                "GitHub would not let this comment on #{number} of {} (403){detail}. The conversation is locked.",
                repo.full
            )
        })
    };
    let answer = github(
        ctx,
        "POST",
        &format!("{}/issues/{number}/comments", repo.path),
        GitHubOptions::what(format!(
            "comment on pull request #{number} of {}",
            repo.full
        ))
        .json(serde_json::json!({"body": body}))
        .explain(&explain),
    )
    .await?;
    let created = expect_body(&answer.body, "html_url", "a comment")?;
    Outcome::ok(
        format!("Commented on pull request #{number} in {}.", repo.full),
        Fields::new()
            .set("id", get(created, "id").clone())
            .set("html_url", get(created, "html_url").clone()),
    )
}

handler!(run);
