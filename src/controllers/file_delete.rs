//! Delete a file from a repository, as a commit.
use crate::handler;
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use regex::Regex;

use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg, optional_text, required_text};
use crate::models::body::{Fields, get};
use crate::models::shapes::expect_body;
use crate::services::contents::{blob_sha, contents_path};
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_delete",
        description: "Delete one file from a GitHub repository, as one commit on a branch (default: the default branch). Destructive: the file stays in history but leaves the branch, so confirm with the owner first. Reads the file's current sha first unless one is passed.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: DESTROY,
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
    "path": {
      "type": "string",
      "description": "Path of the file to delete."
    },
    "message": {
      "type": "string",
      "description": "Commit message."
    },
    "branch": {
      "description": "Branch to commit to. Default: the default branch.",
      "type": "string"
    },
    "sha": {
      "description": "Blob sha of the version being deleted (from file_read).",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "path",
    "message"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "deleted": {
      "type": "boolean"
    },
    "path": {
      "type": "string"
    },
    "commit_sha": {
      "type": "string"
    }
  },
  "required": [
    "deleted",
    "path",
    "commit_sha"
  ],
  "additionalProperties": false
}"#;

static SHA_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bsha\b").unwrap());

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let message = required_text(arg(args, "message"), "message")?;
    let branch = optional_ref(arg(args, "branch"), "branch")?;
    let given = optional_text(arg(args, "sha"), "sha")?;

    let sha = match given {
        Some(sha) => sha,
        None => {
            blob_sha(
                ctx,
                &repo,
                &file,
                branch.as_deref(),
                "There is nothing to delete.",
            )
            .await?
        }
    };
    let explain = |status: u16, detail: &str| {
        (status == 409 || (status == 422 && SHA_WORD.is_match(detail))).then(|| {
            format!(
                "GitHub could not delete '{}' from {} ({status}){detail}. The file changed since its sha was read; read it again with file_read before deleting.",
                file.path, repo.full
            )
        })
    };
    let json = Fields::new()
        .set("message", message)
        .set("sha", sha)
        .opt("branch", branch.clone())
        .into_value();
    let answer = github(
        ctx,
        "DELETE",
        &contents_path(&repo, &file),
        GitHubOptions::what(format!("delete '{}' from {}", file.path, repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;

    let commit = expect_body(get(&answer.body, "commit"), "sha", "a commit")?;
    let on = branch.unwrap_or_else(|| "the default branch".to_string());
    Outcome::ok(
        format!(
            "Deleted {} from {} on {on} (commit {}).",
            file.path,
            repo.full,
            short(get(commit, "sha"))
        ),
        Fields::new()
            .set("deleted", true)
            .set("path", file.path.clone())
            .set("commit_sha", get(commit, "sha").clone()),
    )
}

handler!(run);
