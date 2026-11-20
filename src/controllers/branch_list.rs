//! List a repository's branches.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::parse_repo;
use crate::models::args::{Args, arg, maybe_flag};
use crate::models::body::{Fields, at, boolean, get, list, nn};
use crate::models::paging::{page_query, paging};

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_list",
        description: "List the branches of a GitHub repository, with the sha at each tip and whether it is protected. Paged. Use it to see what branches exist before creating, merging or deleting one.",
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
    "protected": {
      "description": "true lists only protected branches, false only unprotected ones. Omit for all.",
      "type": "boolean"
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
          },
          "protected": {
            "type": "boolean"
          }
        },
        "required": [
          "name",
          "sha",
          "protected"
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
    let only_protected = maybe_flag(arg(args, "protected"), "protected")?;
    let page = page_query(args)?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/branches", repo.path),
        GitHubOptions::what(format!("list the branches of {}", repo.full))
            .query("protected", only_protected)
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|branch| {
            Fields::new()
                .set("name", get(branch, "name").clone())
                .set("sha", nn(at(branch, "commit.sha"), ""))
                .set("protected", boolean(get(branch, "protected")))
                .into_value()
        })
        .collect();
    let kind = match only_protected {
        None => "",
        Some(true) => "protected ",
        Some(false) => "unprotected ",
    };
    let summary = if items.is_empty() {
        format!("{} has no {kind}branches on page {}.", repo.full, page.page)
    } else {
        let noun = if items.len() == 1 {
            "branch"
        } else {
            "branches"
        };
        format!(
            "{} {kind}{noun} of {} on page {}.",
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
