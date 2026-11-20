//! Read one commit.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::{MAX_DIFF_CHARS, MAX_FILES_SHOWN};
use crate::models::address::{parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg, optional_flag};
use crate::models::body::{Fields, at, count, file_rows, get, list, nn, present, text};
use crate::models::shapes::expect_body;
use crate::utils::text::{first_line, head, short};

pub fn def() -> ToolDef {
    ToolDef {
        name: "commit_read",
        description: "Read one commit of a GitHub repository: its message, author, date, parents, the files it changed with their line counts, and optionally its diff (cut at 20,000 characters). Use it to see what a commit did. Defaults to the tip of the default branch.",
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
    "ref": {
      "description": "Commit sha, branch or tag. Default: the default branch's tip.",
      "type": "string"
    },
    "diff": {
      "description": "true also returns the commit's diff. Default false.",
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
    "sha": {
      "type": "string"
    },
    "message": {
      "type": "string"
    },
    "author": {
      "type": "string"
    },
    "date": {
      "type": "string"
    },
    "parents": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "additions": {
      "type": "number"
    },
    "deletions": {
      "type": "number"
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
    "diff": {
      "type": "string"
    },
    "truncated": {
      "type": "boolean"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "sha",
    "message",
    "parents",
    "additions",
    "deletions",
    "file_count",
    "files",
    "truncated"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let reference = if arg(args, "ref").is_null() {
        "HEAD".to_string()
    } else {
        ref_name(arg(args, "ref"), "ref")?
    };
    let want_diff = optional_flag(arg(args, "diff"), "diff", false)?;
    let path = format!("{}/commits/{}", repo.path, ref_path(&reference));

    let answer = github(
        ctx,
        "GET",
        &path,
        GitHubOptions::what(format!("read the commit '{reference}' of {}", repo.full)),
    )
    .await?;
    let body = expect_body(&answer.body, "sha", "a commit")?;

    let listed = list(get(body, "files"));
    let files = file_rows(listed, MAX_FILES_SHOWN);
    let mut truncated = listed.len() > MAX_FILES_SHOWN;

    let mut diff: Option<String> = None;
    if want_diff {
        let explain = |status: u16, _: &str| {
            (status >= 500).then(|| {
                format!(
                    "GitHub could not produce the diff of '{reference}' in {} ({status}). Very large diffs time out; read the changed files from the commit without the diff, or compare a smaller range.",
                    repo.full
                )
            })
        };
        let answer = github(
            ctx,
            "GET",
            &path,
            GitHubOptions::what(format!(
                "read the diff of the commit '{reference}' of {}",
                repo.full
            ))
            .accept("application/vnd.github.diff")
            .raw()
            .explain(&explain),
        )
        .await?;
        let (shown, cut) = head(answer.body.as_str().unwrap_or(""), MAX_DIFF_CHARS);
        diff = Some(shown);
        truncated = truncated || cut;
    }

    let subject = first_line(at(body, "commit.message"));
    let author = text(at(body, "commit.author.name"));
    let by = author
        .as_ref()
        .map(|a| format!(" by {a}"))
        .unwrap_or_default();
    let subject = if subject.is_empty() {
        "(no message)".to_string()
    } else {
        subject
    };
    let parents: Vec<Value> = list(get(body, "parents"))
        .iter()
        .map(|parent| get(parent, "sha").clone())
        .collect();
    Outcome::ok(
        format!("Commit {}{by}: {subject}", short(get(body, "sha"))),
        Fields::new()
            .set("sha", get(body, "sha").clone())
            .set("message", nn(at(body, "commit.message"), ""))
            .opt("author", author)
            .opt("date", present(at(body, "commit.author.date")))
            .set("parents", parents)
            .set("additions", count(at(body, "stats.additions")))
            .set("deletions", count(at(body, "stats.deletions")))
            .set("file_count", listed.len())
            .set("files", files)
            .opt("diff", diff)
            .set("truncated", truncated)
            .opt("html_url", present(get(body, "html_url"))),
    )
}

handler!(run);
