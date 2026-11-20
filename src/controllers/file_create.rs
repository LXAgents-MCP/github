//! Create a file in a repository, as a commit.
use crate::handler;
use crate::models::schemas::{WRITE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use regex::Regex;

use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg, required_raw_text, required_text};
use crate::models::body::{Fields, at, get, nn};
use crate::models::shapes::expect_body;
use crate::services::contents::{contents_path, encode_content};
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_create",
        description: "Create a new file in a GitHub repository, as one commit on a branch (default: the default branch). Fails if the file already exists; use file_update to change it. Content up to 1 MB.",
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
    "path": {
      "type": "string",
      "description": "Path of the new file, e.g. 'docs/notes.md'. Missing folders are created."
    },
    "content": {
      "type": "string",
      "description": "The file's full content. An empty string creates an empty file."
    },
    "message": {
      "type": "string",
      "description": "Commit message, e.g. 'docs: add notes'."
    },
    "branch": {
      "description": "Branch to commit to. Default: the default branch.",
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

    let encoded = encode_content(&content)?;
    let explain = |status: u16, detail: &str| {
        if status == 422 && SHA_WORD.is_match(detail) {
            let on = branch
                .as_ref()
                .map(|b| format!("on {b}"))
                .unwrap_or_else(|| "on that branch".to_string());
            return Some(format!(
                "GitHub could not create '{}' in {} (422){detail}. The file already exists {on}; use file_update to change it.",
                file.path, repo.full
            ));
        }
        if status == 409 {
            return Some(format!(
                "GitHub could not create '{}' in {} (409){detail}. The branch moved while the commit was made; try again.",
                file.path, repo.full
            ));
        }
        None
    };
    let json = Fields::new()
        .set("message", message)
        .set("content", encoded)
        .opt("branch", branch.clone())
        .into_value();
    let answer = github(
        ctx,
        "PUT",
        &contents_path(&repo, &file),
        GitHubOptions::what(format!("create '{}' in {}", file.path, repo.full))
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
            "Created {} in {} on {on} (commit {}).",
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
