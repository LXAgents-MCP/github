//! List a repository's GitHub Actions workflows.
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
use crate::models::body::{Fields, get, is_int, list, plural, s};
use crate::models::paging::{page_query, paging};

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_list",
        description: "List the GitHub Actions workflows of a repository with their id, name, file path and state. Paged. Use it to find the workflow to run or to follow; the id or the file name, such as 'ci.yml', identifies it.",
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
          "name": {
            "type": "string"
          },
          "path": {
            "type": "string"
          },
          "state": {
            "type": "string"
          }
        },
        "required": [
          "id",
          "name",
          "path",
          "state"
        ],
        "additionalProperties": false
      }
    },
    "total_count": {
      "type": "number"
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
    "total_count",
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
        &format!("{}/actions/workflows", repo.path),
        GitHubOptions::what(format!("list the workflows of {}", repo.full))
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;
    let body = &answer.body;

    let items: Vec<Value> = list(get(body, "workflows"))
        .iter()
        .map(|workflow| {
            Fields::new()
                .set("id", get(workflow, "id").clone())
                .set("name", get(workflow, "name").clone())
                .set("path", get(workflow, "path").clone())
                .set("state", get(workflow, "state").clone())
                .into_value()
        })
        .collect();
    let total = if is_int(get(body, "total_count")) {
        get(body, "total_count").clone()
    } else {
        items.len().into()
    };

    let summary = if items.is_empty() {
        format!("{} has no workflows on page {}.", repo.full, page.page)
    } else {
        format!(
            "{} of {} {} of {}, on page {}.",
            items.len(),
            s(&total),
            plural(&total, "workflow", "workflows"),
            repo.full,
            page.page
        )
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("items", items)
            .set("total_count", total)
            .merge(paging(page, &answer.headers)),
    )
}

handler!(run);
