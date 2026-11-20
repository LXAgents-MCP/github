//! Open a pull request.
use crate::handler;
use crate::models::schemas::{WRITE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use crate::models::address::{optional_ref, parse_repo, ref_name};
use crate::models::args::{Args, arg, maybe_flag, raw_text, required_text};
use crate::models::body::{Fields, boolean, get, s, truthy};
use crate::models::shapes::expect_body;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_open",
        description: "Open a pull request in a GitHub repository from a branch into a base branch (default: the default branch), optionally as a draft. For a branch in a fork, give the head as 'owner:branch'. Fails if there are no commits between them or one is already open.",
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
    "title": {
      "type": "string",
      "description": "Pull request title."
    },
    "head": {
      "type": "string",
      "description": "Branch with the changes, e.g. 'feat/login', or 'owner:branch' for a fork."
    },
    "base": {
      "description": "Branch to merge into. Default: the repository's default branch.",
      "type": "string"
    },
    "body": {
      "description": "Description, in Markdown.",
      "type": "string"
    },
    "draft": {
      "description": "true opens it as a draft. Default false.",
      "type": "boolean"
    }
  },
  "required": [
    "repo",
    "title",
    "head"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "title": {
      "type": "string"
    },
    "state": {
      "type": "string"
    },
    "draft": {
      "type": "boolean"
    },
    "head": {
      "type": "string"
    },
    "base": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    }
  },
  "required": [
    "number",
    "title",
    "state",
    "draft",
    "head",
    "base",
    "html_url"
  ],
  "additionalProperties": false
}"#;

static LOGIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,38}$").unwrap());

/// The head of a pull request: a branch here, or `owner:branch` for one in a fork.
fn head_ref(value: &Value) -> Result<String, Refused> {
    let text = required_text(value, "head")?;
    let Some(colon) = text.find(':') else {
        return ref_name(&json!(text), "head");
    };
    let owner = &text[..colon];
    if !LOGIN.is_match(owner) {
        return Err(Refused(format!(
            "`head` '{text}' is not 'branch' or 'owner:branch'."
        )));
    }
    Ok(format!(
        "{owner}:{}",
        ref_name(&json!(&text[colon + 1..]), "head")?
    ))
}

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let title = required_text(arg(args, "title"), "title")?;
    let head = head_ref(arg(args, "head"))?;
    let given = optional_ref(arg(args, "base"), "base")?;
    let body = raw_text(arg(args, "body"), "body")?;
    let draft = maybe_flag(arg(args, "draft"), "draft")?;

    let base = match given {
        Some(base) => base,
        None => {
            let answer = github(
                ctx,
                "GET",
                &repo.path,
                GitHubOptions::what(format!("find the default branch of {}", repo.full)),
            )
            .await?;
            match get(&answer.body, "default_branch").as_str() {
                Some(base) => base.to_string(),
                None => {
                    return Err(Refused(format!(
                        "Could not find the default branch of {}; pass `base`.",
                        repo.full
                    )));
                }
            }
        }
    };

    let explain = |status: u16, detail: &str| {
        if status != 422 {
            return None;
        }
        let lower = detail.to_lowercase();
        if lower.contains("already exists") {
            return Some(format!(
                "GitHub would not open a pull request from '{head}' into '{base}' in {} (422){detail}. One is already open; find it with pr_read.",
                repo.full
            ));
        }
        if lower.contains("no commits between") {
            return Some(format!(
                "GitHub would not open a pull request from '{head}' into '{base}' in {} (422){detail}. The branches have no difference to propose.",
                repo.full
            ));
        }
        None
    };
    let json = Fields::new()
        .set("title", title)
        .set("head", head.clone())
        .set("base", base.clone())
        .opt("body", body)
        .opt("draft", draft)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!("{}/pulls", repo.path),
        GitHubOptions::what(format!(
            "open a pull request from '{head}' into '{base}' in {}",
            repo.full
        ))
        .json(json)
        .explain(&explain),
    )
    .await?;
    let created = expect_body(&answer.body, "html_url", "a pull request")?;

    let kind = if truthy(get(created, "draft")) {
        "draft "
    } else {
        ""
    };
    Outcome::ok(
        format!(
            "Opened {kind}pull request #{} in {}: {head} into {base}.",
            s(get(created, "number")),
            repo.full
        ),
        Fields::new()
            .set("number", get(created, "number").clone())
            .set("title", get(created, "title").clone())
            .set("state", get(created, "state").clone())
            .set("draft", boolean(get(created, "draft")))
            .set("head", head.clone())
            .set("base", base.clone())
            .set("html_url", get(created, "html_url").clone()),
    )
}

handler!(run);
