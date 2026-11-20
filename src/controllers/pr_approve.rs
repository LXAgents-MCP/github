//! Approve a pull request.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, optional_text, raw_text, required_int};
use crate::models::body::{Fields, get, present, s};
use crate::models::schemas::{WRITE, schema};
use crate::models::shapes::expect_body;
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_approve",
        description: "Approve a pull request in a GitHub repository as the token's user, with an optional message. A person cannot approve their own pull request. Each call records another approval review.",
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
      "description": "A message to go with the approval.",
      "type": "string"
    },
    "sha": {
      "description": "The head commit being approved; the approval fails if the branch has moved on. Default: the latest.",
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
    "id": {
      "type": "number"
    },
    "state": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "state"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let body = raw_text(arg(args, "body"), "body")?.filter(|body| !body.trim().is_empty());
    let sha = optional_text(arg(args, "sha"), "sha")?;

    let explain = |status: u16, detail: &str| {
        (status == 422 && detail.to_lowercase().contains("own pull request")).then(|| {
            format!(
                "GitHub would not record the approval on #{number} of {} (422){detail}. A person cannot approve their own pull request; it needs a different reviewer.",
                repo.full
            )
        })
    };
    let json = Fields::new()
        .set("event", "APPROVE")
        .opt("body", body)
        .opt("commit_id", sha)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!("{}/pulls/{number}/reviews", repo.path),
        GitHubOptions::what(format!("approve pull request #{number} of {}", repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;
    let review = expect_body(&answer.body, "state", "a review")?;
    Outcome::ok(
        format!(
            "Approved pull request #{number} in {} (review state {}).",
            repo.full,
            s(get(review, "state"))
        ),
        Fields::new()
            .set("id", get(review, "id").clone())
            .set("state", get(review, "state").clone())
            .opt("html_url", present(get(review, "html_url"))),
    )
}

handler!(run);
