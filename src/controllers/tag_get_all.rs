//! List a repository's tags.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::parse_repo;
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, at, get, list, nn};
use crate::models::paging::{page_query, paging};

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_get_all",
        description: "List the tags of a GitHub repository with the commit each points at. Paged, in the order GitHub returns them (newest first for version-like names). Use it to see what has been tagged or released.",
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
          "name": {
            "type": "string"
          },
          "sha": {
            "type": "string"
          }
        },
        "required": [
          "name",
          "sha"
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
    let page = page_query(args)?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/tags", repo.path),
        GitHubOptions::what(format!("list the tags of {}", repo.full))
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|tag| {
            Fields::new()
                .set("name", get(tag, "name").clone())
                .set("sha", nn(at(tag, "commit.sha"), ""))
                .into_value()
        })
        .collect();
    let summary = if items.is_empty() {
        format!("{} has no tags on page {}.", repo.full, page.page)
    } else {
        let noun = if items.len() == 1 { "tag" } else { "tags" };
        format!(
            "{} {noun} of {} on page {}.",
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
