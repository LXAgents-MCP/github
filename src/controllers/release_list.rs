//! List a repository's releases.
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
use crate::models::body::{Fields, boolean, get, list, present, text};
use crate::models::paging::{page_query, paging};

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_list",
        description: "List the releases of a GitHub repository, newest first, including drafts when the token has push access. Paged. Use release_read for one release's notes and assets.",
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
          "id": {
            "type": "number"
          },
          "tag": {
            "type": "string"
          },
          "name": {
            "type": "string"
          },
          "draft": {
            "type": "boolean"
          },
          "prerelease": {
            "type": "boolean"
          },
          "published_at": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "tag",
          "draft",
          "prerelease"
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
        &format!("{}/releases", repo.path),
        GitHubOptions::what(format!("list the releases of {}", repo.full))
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|release| {
            Fields::new()
                .set("id", get(release, "id").clone())
                .set("tag", get(release, "tag_name").clone())
                .opt("name", text(get(release, "name")))
                .set("draft", boolean(get(release, "draft")))
                .set("prerelease", boolean(get(release, "prerelease")))
                .opt("published_at", present(get(release, "published_at")))
                .into_value()
        })
        .collect();
    let summary = if items.is_empty() {
        format!("{} has no releases on page {}.", repo.full, page.page)
    } else {
        let noun = if items.len() == 1 {
            "release"
        } else {
            "releases"
        };
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
