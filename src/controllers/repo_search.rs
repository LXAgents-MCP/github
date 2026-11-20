//! Search repositories across GitHub.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::args::{Args, arg, one_of, required_text};
use crate::models::body::{Fields, count, get, is_int, list, text};
use crate::models::paging::{page_query, paging};

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_search",
        description: "Search GitHub repositories with GitHub's search syntax, such as 'org:LXAgents-MCP mcp in:name' or 'topic:mcp language:javascript'. Use it to find a repository when the owner is not known. GitHub serves at most the first 1000 matches and limits search to 30 requests a minute.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: READ,
    }
}

const INPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Keywords and qualifiers, e.g. 'mcp in:name org:LXAgents-MCP' or 'user:octocat archived:false'."
    },
    "sort": {
      "description": "stars, forks, help-wanted-issues or updated. Default: best match.",
      "type": "string",
      "enum": [
        "stars",
        "forks",
        "help-wanted-issues",
        "updated"
      ]
    },
    "order": {
      "description": "asc or desc (default). Only applies when sort is given.",
      "type": "string",
      "enum": [
        "asc",
        "desc"
      ]
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
    "query"
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
          "full_name": {
            "type": "string"
          },
          "description": {
            "type": "string"
          },
          "stars": {
            "type": "number"
          },
          "language": {
            "type": "string"
          },
          "url": {
            "type": "string"
          }
        },
        "required": [
          "full_name",
          "stars",
          "url"
        ],
        "additionalProperties": false
      }
    },
    "total_count": {
      "type": "number"
    },
    "incomplete": {
      "type": "boolean"
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
    "incomplete",
    "page",
    "per_page",
    "has_more"
  ],
  "additionalProperties": false
}"#;

const SORTS: &[&str] = &["stars", "forks", "help-wanted-issues", "updated"];
const ORDERS: &[&str] = &["asc", "desc"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let query = required_text(arg(args, "query"), "query")?;
    let sort = one_of(arg(args, "sort"), "sort", SORTS)?;
    let order = one_of(arg(args, "order"), "order", ORDERS)?;
    let page = page_query(args)?;

    let order = if sort.is_some() { order } else { None };
    let answer = github(
        ctx,
        "GET",
        "/search/repositories",
        GitHubOptions::what(format!("search repositories for '{query}'"))
            .query("q", query.clone())
            .query("sort", sort)
            .query("order", order)
            .query("page", page.page)
            .query("per_page", page.per_page),
    )
    .await?;
    let body = &answer.body;

    let items: Vec<Value> = list(get(body, "items"))
        .iter()
        .map(|repo| {
            Fields::new()
                .set("full_name", get(repo, "full_name").clone())
                .opt("description", text(get(repo, "description")))
                .set("stars", count(get(repo, "stargazers_count")))
                .opt("language", text(get(repo, "language")))
                .set("url", get(repo, "html_url").clone())
                .into_value()
        })
        .collect();
    let total = if is_int(get(body, "total_count")) {
        get(body, "total_count").as_f64().unwrap_or(0.0) as i64
    } else {
        items.len() as i64
    };

    let summary = if total == 0 {
        format!("No repositories match '{query}'.")
    } else {
        let noun = if total == 1 {
            "repository matches"
        } else {
            "repositories match"
        };
        format!("{total} {noun} '{query}'; showing {}.", items.len())
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("items", items)
            .set("total_count", total)
            .set(
                "incomplete",
                get(body, "incomplete_results") == &Value::Bool(true),
            )
            .merge(paging(page, &answer.headers)),
    )
}

handler!(run);
