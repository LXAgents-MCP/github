//! Lock or unlock an issue's conversation.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::json;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, one_of, optional_flag, required_int};
use crate::models::body::Fields;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_lock",
        description: "Lock an issue or pull request in a GitHub repository so only collaborators can comment, with an optional reason, or unlock it. Needs push access.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: UPDATE,
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
      "description": "Issue or pull request number."
    },
    "locked": {
      "description": "true locks (default), false unlocks.",
      "type": "boolean"
    },
    "reason": {
      "description": "Why it is locked: off-topic, too heated, resolved or spam. Only when locking.",
      "type": "string",
      "enum": [
        "off-topic",
        "too heated",
        "resolved",
        "spam"
      ]
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
    "number": {
      "type": "number"
    },
    "locked": {
      "type": "boolean"
    },
    "reason": {
      "type": "string"
    }
  },
  "required": [
    "number",
    "locked"
  ],
  "additionalProperties": false
}"#;

const REASONS: &[&str] = &["off-topic", "too heated", "resolved", "spam"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let lock = optional_flag(arg(args, "locked"), "locked", true)?;
    let reason = one_of(arg(args, "reason"), "reason", REASONS)?;
    let path = format!("{}/issues/{number}/lock", repo.path);

    if lock {
        let mut options = GitHubOptions::what(format!("lock #{number} of {}", repo.full));
        if let Some(reason) = &reason {
            options = options.json(json!({"lock_reason": reason}));
        }
        github(ctx, "PUT", &path, options).await?;
    } else {
        github(
            ctx,
            "DELETE",
            &path,
            GitHubOptions::what(format!("unlock #{number} of {}", repo.full)),
        )
        .await?;
    }

    let summary = if lock {
        let reason = reason
            .as_ref()
            .map(|r| format!(" as {r}"))
            .unwrap_or_default();
        format!("Locked #{number} in {}{reason}.", repo.full)
    } else {
        format!("Unlocked #{number} in {}.", repo.full)
    };
    Outcome::ok(
        summary,
        Fields::new()
            .set("number", number)
            .set("locked", lock)
            .opt("reason", if lock { reason } else { None }),
    )
}

handler!(run);
