//! Compare two refs.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::{MAX_COMMITS_SHOWN, MAX_FILES_SHOWN};
use crate::models::address::{parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, at, count, file_rows, get, list, nn, plural, present, s, text};
use crate::models::shapes::expect_body;
use crate::utils::text::first_line;

pub fn def() -> ToolDef {
    ToolDef {
        name: "commit_compare",
        description: "Compare two branches, tags or commits of a GitHub repository: how far ahead or behind the head is of the base, the most recent 50 commits between them, and the changed files with line counts. Use it to see what a branch would bring in before merging.",
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
    "base": {
      "type": "string",
      "description": "The ref to compare from, e.g. 'main'."
    },
    "head": {
      "type": "string",
      "description": "The ref to compare, e.g. 'feat/login'."
    }
  },
  "required": [
    "repo",
    "base",
    "head"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "status": {
      "type": "string"
    },
    "ahead_by": {
      "type": "number"
    },
    "behind_by": {
      "type": "number"
    },
    "total_commits": {
      "type": "number"
    },
    "commits": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "sha": {
            "type": "string"
          },
          "message": {
            "type": "string"
          },
          "author": {
            "type": "string"
          }
        },
        "required": [
          "sha",
          "message"
        ],
        "additionalProperties": false
      }
    },
    "file_count": {
      "type": "number"
    },
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "filename": {
            "type": "string"
          },
          "status": {
            "type": "string"
          },
          "additions": {
            "type": "number"
          },
          "deletions": {
            "type": "number"
          }
        },
        "required": [
          "filename",
          "status",
          "additions",
          "deletions"
        ],
        "additionalProperties": false
      }
    },
    "truncated": {
      "type": "boolean"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "status",
    "ahead_by",
    "behind_by",
    "total_commits",
    "commits",
    "file_count",
    "files",
    "truncated"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let base = ref_name(arg(args, "base"), "base")?;
    let head = ref_name(arg(args, "head"), "head")?;

    let answer = github(
        ctx,
        "GET",
        &format!(
            "{}/compare/{}...{}",
            repo.path,
            ref_path(&base),
            ref_path(&head)
        ),
        GitHubOptions::what(format!("compare '{head}' with '{base}' in {}", repo.full)),
    )
    .await?;
    let body = expect_body(&answer.body, "status", "a comparison")?;

    let all = list(get(body, "commits"));
    let listed = list(get(body, "files"));
    // Commits arrive oldest first, and the newest are the ones a reader came for.
    let commits: Vec<Value> = all[all.len().saturating_sub(MAX_COMMITS_SHOWN)..]
        .iter()
        .map(|commit| {
            Fields::new()
                .set("sha", get(commit, "sha").clone())
                .set("message", first_line(at(commit, "commit.message")))
                .opt("author", text(at(commit, "commit.author.name")))
                .into_value()
        })
        .collect();
    let files = file_rows(listed, MAX_FILES_SHOWN);
    let ahead = count(get(body, "ahead_by"));
    let behind = count(get(body, "behind_by"));

    let status = get(body, "status").as_str().unwrap_or("");
    let sentence = match status {
        "identical" => format!("{head} and {base} are identical."),
        "ahead" => format!(
            "{head} is {} {} ahead of {base}.",
            s(&ahead),
            plural(&ahead, "commit", "commits")
        ),
        "behind" => format!(
            "{head} is {} {} behind {base}.",
            s(&behind),
            plural(&behind, "commit", "commits")
        ),
        "diverged" => format!(
            "{head} has diverged from {base}: {} ahead, {} behind.",
            s(&ahead),
            s(&behind)
        ),
        other => format!("{head} compared with {base}: {other}."),
    };
    Outcome::ok(
        sentence,
        Fields::new()
            .set("status", get(body, "status").clone())
            .set("ahead_by", ahead)
            .set("behind_by", behind)
            .set("total_commits", nn(get(body, "total_commits"), all.len()))
            .set("commits", commits)
            .set("file_count", listed.len())
            .set("files", files)
            .set(
                "truncated",
                all.len() > MAX_COMMITS_SHOWN || listed.len() > MAX_FILES_SHOWN,
            )
            .opt("html_url", present(get(body, "html_url"))),
    )
}

handler!(run);
