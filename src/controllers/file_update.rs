//! Replace a file's content in a repository, as a commit.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use regex::Regex;

use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg, optional_text, required_raw_text, required_text};
use crate::models::body::{Fields, at, get, nn};
use crate::models::shapes::expect_body;
use crate::services::contents::{blob_sha, contents_path, encode_content};
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_update",
        description: "Replace the content of an existing file in a GitHub repository, as one commit on a branch (default: the default branch). Reads the file's current sha first unless one is passed. Use file_create for a new file.",
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
    "path": {
      "type": "string",
      "description": "Path of the existing file."
    },
    "content": {
      "type": "string",
      "description": "The file's new full content - not a patch."
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
      "description": "Blob sha of the version being replaced (from file_read). Pass it to fail if the file changed since.",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "path",
    "content",
    "message"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "path": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "commit_sha": {
      "type": "string"
    },
    "branch": {
      "type": "string"
    }
  },
  "required": [
    "path",
    "sha",
    "commit_sha"
  ],
  "additionalProperties": false
}"#;

static SHA_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bsha\b").unwrap());

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let content = required_raw_text(arg(args, "content"), "content", true)?;
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
                "Use file_create to make it.",
            )
            .await?
        }
    };
    let encoded = encode_content(&content)?;
    let explain = |status: u16, detail: &str| {
        (status == 409 || (status == 422 && SHA_WORD.is_match(detail))).then(|| {
            format!(
                "GitHub could not update '{}' in {} ({status}){detail}. The file changed since its sha was read; read it again with file_read and apply the change to that version.",
                file.path, repo.full
            )
        })
    };
    let json = Fields::new()
        .set("message", message)
        .set("content", encoded)
        .set("sha", sha)
        .opt("branch", branch.clone())
        .into_value();
    let answer = github(
        ctx,
        "PUT",
        &contents_path(&repo, &file),
        GitHubOptions::what(format!("update '{}' in {}", file.path, repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;

    let commit = expect_body(get(&answer.body, "commit"), "sha", "a commit")?;
    let on = branch
        .clone()
        .unwrap_or_else(|| "the default branch".to_string());
    Outcome::ok(
        format!(
            "Updated {} in {} on {on} (commit {}).",
            file.path,
            repo.full,
            short(get(commit, "sha"))
        ),
        Fields::new()
            .set("path", file.path.clone())
            .set("sha", nn(at(&answer.body, "content.sha"), ""))
            .set("commit_sha", get(commit, "sha").clone())
            .opt("branch", branch),
    )
}

handler!(run);
