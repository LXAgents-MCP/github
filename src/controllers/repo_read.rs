//! Read one repository.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, arg};
use crate::models::schemas::{READ, schema};
use crate::models::shapes::{describe_repository, expect_body, repository_fields};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_read",
        description: "Read one GitHub repository: description, visibility, default branch, star, fork and issue counts, language, license, topics and parent. Use it to learn about a repository before acting on it.",
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
    "full_name": {
      "type": "string"
    },
    "description": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    },
    "clone_url": {
      "type": "string"
    },
    "visibility": {
      "type": "string"
    },
    "default_branch": {
      "type": "string"
    },
    "fork": {
      "type": "boolean"
    },
    "archived": {
      "type": "boolean"
    },
    "stars": {
      "type": "number"
    },
    "forks": {
      "type": "number"
    },
    "open_issues": {
      "type": "number"
    },
    "language": {
      "type": "string"
    },
    "license": {
      "type": "string"
    },
    "topics": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "pushed_at": {
      "type": "string"
    },
    "parent": {
      "type": "string"
    }
  },
  "required": [
    "full_name",
    "html_url",
    "clone_url",
    "visibility",
    "fork",
    "archived",
    "stars",
    "forks",
    "open_issues",
    "topics"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let answer = github(
        ctx,
        "GET",
        &repo.path,
        GitHubOptions::what(format!("read the repository {}", repo.full)),
    )
    .await?;

    let fields = repository_fields(expect_body(&answer.body, "full_name", "a repository")?);
    Outcome::ok(describe_repository(&fields), fields)
}

handler!(run);
