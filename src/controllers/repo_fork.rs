//! Fork a repository.
use crate::handler;
use crate::models::address::{REPO_NAME, parse_repo};
use crate::models::args::{Args, arg, maybe_flag, optional_text};
use crate::models::body::Fields;
use crate::models::schemas::{WRITE, schema};
use crate::models::shapes::{expect_body, repository_fields};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_fork",
        description: "Fork a GitHub repository into the token owner's account or an organization. GitHub copies it in the background, so the fork may take a moment to be readable. Use it to work on a repository the owner cannot push to.",
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
    "organization": {
      "description": "Organization to fork into. Omit to fork into the token owner's account.",
      "type": "string"
    },
    "name": {
      "description": "Name for the fork. Omit to keep the source's name.",
      "type": "string"
    },
    "default_branch_only": {
      "description": "true copies only the default branch. Default false.",
      "type": "boolean"
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
    let organization = optional_text(arg(args, "organization"), "organization")?;
    let name = optional_text(arg(args, "name"), "name")?;
    if let Some(name) = &name
        && (!REPO_NAME.is_match(name) || name == "." || name == "..")
    {
        return Err(Refused(format!(
            "`name` '{name}' is not a valid GitHub repository name."
        )));
    }
    let default_branch_only = maybe_flag(arg(args, "default_branch_only"), "default_branch_only")?;

    let json = Fields::new()
        .opt("organization", organization)
        .opt("name", name)
        .opt("default_branch_only", default_branch_only)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!("{}/forks", repo.path),
        GitHubOptions::what(format!("fork {}", repo.full)).json(json),
    )
    .await?;

    let fields = repository_fields(expect_body(&answer.body, "full_name", "a repository")?);
    let summary = format!(
        "Forking {} into {} has started. GitHub finishes the copy in the background, so give it a moment before reading it.",
        repo.full,
        crate::utils::render::js_string(&fields["full_name"])
    );
    Outcome::ok(summary, fields)
}

handler!(run);
