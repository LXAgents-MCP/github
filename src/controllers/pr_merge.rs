//! Merge a pull request.
use crate::handler;
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, one_of_or, optional_text, raw_text, required_int};
use crate::models::body::{Fields, get, present, s, truthy};
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_merge",
        description: "Merge a pull request in a GitHub repository, by merge commit, squash or rebase. Destructive: it moves the base branch, so confirm with the owner first. Pass `sha` to merge only if the branch is still at the commit that was reviewed.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: DESTROY,
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Pull request number."
    },
    "method": {
      "description": "merge (default), squash or rebase. The repository may disable some.",
      "type": "string",
      "enum": [
        "merge",
        "squash",
        "rebase"
      ]
    },
    "title": {
      "description": "Title for the merge or squash commit. Default: GitHub's.",
      "type": "string"
    },
    "message": {
      "description": "Body for the merge or squash commit. Default: GitHub's.",
      "type": "string"
    },
    "sha": {
      "description": "The head commit the merge must match; a moved branch is refused (409).",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "number"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "merged": {
      "type": "boolean"
    },
    "sha": {
      "type": "string"
    },
    "message": {
      "type": "string"
    }
  },
  "required": [
    "merged"
  ],
  "additionalProperties": false
}"#;

const METHODS: &[&str] = &["merge", "squash", "rebase"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let method = one_of_or(arg(args, "method"), "method", METHODS, "merge")?;
    let title = optional_text(arg(args, "title"), "title")?;
    let message = raw_text(arg(args, "message"), "message")?;
    let sha = optional_text(arg(args, "sha"), "sha")?;

    let explain = |status: u16, detail: &str| match status {
        405 => Some(format!(
            "GitHub would not merge #{number} in {} (405){detail}. It is not mergeable: it may have conflicts, required checks that are failing or pending, a missing approval, or {method} merges may be disabled for the repository. A repository with a merge queue cannot be merged this way.",
            repo.full
        )),
        409 => Some(format!(
            "GitHub did not merge #{number} in {} (409){detail}. The branch moved since the commit that was meant to be merged; read the pull request again with pr_read.",
            repo.full
        )),
        _ => None,
    };
    let json = Fields::new()
        .set("merge_method", method.clone())
        .opt("commit_title", title)
        .opt("commit_message", message)
        .opt("sha", sha)
        .into_value();
    let answer = github(
        ctx,
        "PUT",
        &format!("{}/pulls/{number}/merge", repo.path),
        GitHubOptions::what(format!("merge pull request #{number} of {}", repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;
    let body = &answer.body;
    let said = present(get(body, "message"));

    if get(body, "merged") != &Value::Bool(true) {
        let tail = said
            .as_ref()
            .map(|m| format!(": {}", s(m)))
            .unwrap_or_else(|| ".".to_string());
        return Ok(Outcome {
            summary: format!("GitHub did not merge #{number} in {}{tail}", repo.full),
            fields: Fields::new()
                .set("merged", false)
                .opt("message", said)
                .into_map(),
            ok: false,
        });
    }
    let sha = present(get(body, "sha"));
    let at = if truthy(get(body, "sha")) {
        format!(", at {}", short(get(body, "sha")))
    } else {
        String::new()
    };
    Outcome::ok(
        format!(
            "Merged pull request #{number} in {} by {method}{at}.",
            repo.full
        ),
        Fields::new()
            .set("merged", true)
            .opt("sha", sha)
            .opt("message", said),
    )
}

handler!(run);
