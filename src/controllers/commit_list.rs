//! List the commits of a branch.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg, optional_date, optional_text};
use crate::models::body::{Fields, at, get, list, present, text};
use crate::models::paging::{page_query, paging};
use crate::utils::text::first_line;

pub fn def() -> ToolDef {
    ToolDef {
        name: "commit_list",
        description: "List commits of a GitHub repository, newest first, from a branch, tag or sha, optionally only those touching a path, by an author, or within a date range. Paged. Use commit_read for one commit in full.",
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
      "description": "Branch, tag or commit sha to list back from. Default: the default branch.",
      "type": "string"
    },
    "path": {
      "description": "Only commits that touched this file or directory.",
      "type": "string"
    },
    "author": {
      "description": "Only commits by this GitHub login or email.",
      "type": "string"
    },
    "since": {
      "description": "Only commits after this ISO 8601 date, e.g. 2026-10-01.",
      "type": "string"
    },
    "until": {
      "description": "Only commits before this ISO 8601 date.",
      "type": "string"
    },
    "page": {
      "description": "Page number, from 1. Default 1.",
      "type": "integer",
      "minimum": 1,
      "maximum": 9007199254740991
    },
    "per_page": {
      "description": "Results per page, 1 to 100. Default 30.",
      "type": "integer",
      "minimum": 1,
      "maximum": 100
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
    "items": {
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
          },
          "date": {
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
    "page": {
      "type": "number"
    },
    "per_page": {
      "type": "number"
    },
    "has_more": {
      "type": "boolean"
    }
  },
  "required": [
    "items",
    "page",
    "per_page",
    "has_more"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;
    let path = Some(file_path(arg(args, "path"), "path", true)?.path).filter(|p| !p.is_empty());
    let author = optional_text(arg(args, "author"), "author")?;
    let since = optional_date(arg(args, "since"), "since")?;
    let until = optional_date(arg(args, "until"), "until")?;
    let page = page_query(args)?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/commits", repo.path),
        GitHubOptions::what(format!("list the commits of {}", repo.full))
            .query("sha", reference.clone())
            .query("path", path)
            .query("author", author)
            .query("since", since)
            .query("until", until)
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|commit| {
            Fields::new()
                .set("sha", get(commit, "sha").clone())
                .set("message", first_line(at(commit, "commit.message")))
                .opt("author", text(at(commit, "commit.author.name")))
                .opt("date", present(at(commit, "commit.author.date")))
                .into_value()
        })
        .collect();
    let on = reference
        .as_ref()
        .map(|r| format!(" on {r}"))
        .unwrap_or_default();
    let summary = if items.is_empty() {
        format!(
            "No commits{on} of {} match on page {}.",
            repo.full, page.page
        )
    } else {
        let noun = if items.len() == 1 {
            "commit"
        } else {
            "commits"
        };
        format!(
            "{} {noun}{on} of {} on page {}.",
            items.len(),
            repo.full,
            page.page
        )
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("items", items)
            .merge(paging(page, &answer.headers)),
    )
}

handler!(run);
