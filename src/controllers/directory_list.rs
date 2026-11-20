//! List what is in a directory of a repository.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::MAX_LIST_ENTRIES;
use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, get, is_int};
use crate::services::contents::contents_path;

pub fn def() -> ToolDef {
    ToolDef {
        name: "directory_list",
        description: "List the files and folders in one directory of a GitHub repository, on the default branch or at a branch, tag or commit. Not recursive. Omit the path for the repository root. Use it to find a path before file_read.",
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
    "path": {
      "description": "Directory inside the repository, e.g. 'src/tools'. Omit for the root.",
      "type": "string"
    },
    "ref": {
      "description": "Branch, tag or commit sha. Default: the default branch.",
      "type": "string"
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
    "path": {
      "type": "string"
    },
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string"
          },
          "type": {
            "type": "string"
          },
          "size": {
            "type": "number"
          }
        },
        "required": [
          "name",
          "type",
          "size"
        ],
        "additionalProperties": false
      }
    },
    "count": {
      "type": "number"
    },
    "truncated": {
      "type": "boolean"
    }
  },
  "required": [
    "path",
    "items",
    "count",
    "truncated"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let dir = file_path(arg(args, "path"), "path", true)?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;
    let place = if dir.path.is_empty() {
        "the root".to_string()
    } else {
        format!("'{}'", dir.path)
    };

    let answer = github(
        ctx,
        "GET",
        &contents_path(&repo, &dir),
        GitHubOptions::what(format!("list {place} of {}", repo.full))
            .query("ref", reference.clone()),
    )
    .await?;

    let Value::Array(entries) = &answer.body else {
        return Err(Refused(format!(
            "`path` '{}' is a file in {}, not a directory. Use file_read to read it.",
            dir.path, repo.full
        )));
    };

    let items: Vec<Value> = entries
        .iter()
        .take(MAX_LIST_ENTRIES)
        .map(|entry| {
            let size = get(entry, "size");
            Fields::new()
                .set("name", get(entry, "name").clone())
                .set("type", get(entry, "type").clone())
                .set("size", if is_int(size) { size.clone() } else { 0.into() })
                .into_value()
        })
        .collect();
    let truncated = entries.len() > MAX_LIST_ENTRIES;

    let noun = if entries.len() == 1 {
        "entry"
    } else {
        "entries"
    };
    let at = reference
        .as_ref()
        .map(|r| format!(" at {r}"))
        .unwrap_or_default();
    let tail = if truncated {
        format!("; showing the first {MAX_LIST_ENTRIES}.")
    } else {
        ".".to_string()
    };
    Outcome::ok(
        format!(
            "{} {noun} in {place} of {}{at}{tail}",
            entries.len(),
            repo.full
        ),
        Fields::new()
            .set("path", dir.path.clone())
            .set("items", items)
            .set("count", entries.len())
            .set("truncated", truncated),
    )
}

handler!(run);
