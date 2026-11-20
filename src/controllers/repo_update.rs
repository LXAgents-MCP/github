//! Change a repository's settings.
use crate::handler;
use crate::models::address::{REPO_NAME, optional_ref, parse_repo};
use crate::models::args::{Args, arg, maybe_flag, optional_text, raw_text};
use crate::models::body::Fields;
use crate::models::schemas::{UPDATE, schema};
use crate::models::shapes::{expect_body, repository_fields};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_update",
        description: "Change a GitHub repository's settings: rename it, set its description or homepage, make it private or public, change the default branch, archive or unarchive it, or switch issues on or off. Pass only what should change; at least one setting is required.",
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
    "name": {
      "description": "New repository name. Renaming redirects the old name.",
      "type": "string"
    },
    "description": {
      "description": "New description. Pass an empty string to clear it.",
      "type": "string"
    },
    "homepage": {
      "description": "New homepage URL. Pass an empty string to clear it.",
      "type": "string"
    },
    "private": {
      "description": "true makes the repository private, false makes it public.",
      "type": "boolean"
    },
    "default_branch": {
      "description": "Branch to make the default. It must already exist.",
      "type": "string"
    },
    "archived": {
      "description": "true archives the repository (read-only), false unarchives it.",
      "type": "boolean"
    },
    "has_issues": {
      "description": "Whether the Issues tab is enabled.",
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

    let name = optional_text(arg(args, "name"), "name")?;
    if let Some(name) = &name
        && (!REPO_NAME.is_match(name) || name == "." || name == "..")
    {
        return Err(Refused(format!(
            "`name` '{name}' is not a valid GitHub repository name. Use letters, digits, '-', '_' and '.', at most 100 characters."
        )));
    }
    let description = raw_text(arg(args, "description"), "description")?;
    let homepage = raw_text(arg(args, "homepage"), "homepage")?;
    let is_private = maybe_flag(arg(args, "private"), "private")?;
    let default_branch = optional_ref(arg(args, "default_branch"), "default_branch")?;
    let archived = maybe_flag(arg(args, "archived"), "archived")?;
    let has_issues = maybe_flag(arg(args, "has_issues"), "has_issues")?;

    let json = Fields::new()
        .opt("name", name)
        .opt("description", description)
        .opt("homepage", homepage)
        .opt("private", is_private)
        .opt("default_branch", default_branch)
        .opt("archived", archived)
        .opt("has_issues", has_issues)
        .into_map();
    if json.is_empty() {
        return Err(Refused(
            "Nothing to change: pass at least one of name, description, homepage, private, default_branch, archived or has_issues."
                .to_string(),
        ));
    }
    let changed = json.keys().cloned().collect::<Vec<_>>().join(", ");

    let answer = github(
        ctx,
        "PATCH",
        &repo.path,
        GitHubOptions::what(format!("update the repository {}", repo.full)).json(json.into()),
    )
    .await?;

    let fields = repository_fields(expect_body(&answer.body, "full_name", "a repository")?);
    let summary = format!(
        "Updated {}: {changed}.",
        crate::utils::render::js_string(&fields["full_name"])
    );
    Outcome::ok(summary, fields)
}

handler!(run);
