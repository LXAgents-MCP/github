//! Read one issue, and optionally its discussion.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, optional_flag, required_int};
use crate::models::body::{Fields, at, get, list, nn, present, s, text, truthy};
use crate::models::shapes::{expect_body, issue_fields};

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_read",
        description: "Read one issue of a GitHub repository: title, state, author, labels, assignees, milestone and body, and with `comments: true` its first 30 comments. Also works on a pull request's number, and says when it is one; use pr_read for its branches and diff.",
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Issue number, e.g. 42."
    },
    "comments": {
      "description": "true also returns the first 30 comments. Default false.",
      "type": "boolean"
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
    "title": {
      "type": "string"
    },
    "state": {
      "type": "string"
    },
    "state_reason": {
      "type": "string"
    },
    "author": {
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
    },
    "milestone": {
      "type": "string"
    },
    "comment_count": {
      "type": "number"
    },
    "is_pull_request": {
      "type": "boolean"
    },
    "html_url": {
      "type": "string"
    },
    "created_at": {
      "type": "string"
    },
    "updated_at": {
      "type": "string"
    },
    "closed_at": {
      "type": "string"
    },
    "body": {
      "type": "string"
    },
    "comments": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": {
            "type": "number"
          },
          "author": {
            "type": "string"
          },
          "created_at": {
            "type": "string"
          },
          "body": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "body"
        ],
        "additionalProperties": false
      }
    }
  },
  "required": [
    "number",
    "title",
    "state",
    "labels",
    "assignees",
    "comment_count",
    "is_pull_request",
    "html_url"
  ],
  "additionalProperties": false
}"#;

const COMMENTS_SHOWN: i64 = 30;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let with_comments = optional_flag(arg(args, "comments"), "comments", false)?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/issues/{number}", repo.path),
        GitHubOptions::what(format!("read issue #{number} of {}", repo.full)),
    )
    .await?;
    let fields = issue_fields(expect_body(&answer.body, "html_url", "an issue")?);
    let comment_count = fields["comment_count"].as_f64().unwrap_or(0.0);

    let mut comments: Option<Vec<Value>> = None;
    if with_comments && comment_count > 0.0 {
        let answer = github(
            ctx,
            "GET",
            &format!("{}/issues/{number}/comments", repo.path),
            GitHubOptions::what(format!(
                "read the comments of issue #{number} of {}",
                repo.full
            ))
            .query("per_page", COMMENTS_SHOWN),
        )
        .await?;
        comments = Some(
            list(&answer.body)
                .iter()
                .map(|comment| {
                    Fields::new()
                        .set("id", get(comment, "id").clone())
                        .opt("author", text(at(comment, "user.login")))
                        .opt("created_at", present(get(comment, "created_at")))
                        .set("body", nn(get(comment, "body"), ""))
                        .into_value()
                })
                .collect(),
        );
    }

    let kind = if truthy(&fields["is_pull_request"]) {
        "Pull request"
    } else {
        "Issue"
    };
    let more = match &comments {
        Some(shown) if comment_count > shown.len() as f64 => {
            format!("; showing the first {} comments", shown.len())
        }
        _ => String::new(),
    };
    let summary = format!(
        "{kind} #{number} in {} is {}: {}{more}.",
        repo.full,
        s(&fields["state"]),
        s(&fields["title"])
    );
    Outcome::ok(summary, Fields(fields).opt("comments", comments))
}

handler!(run);
