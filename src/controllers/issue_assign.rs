//! Change who an issue is assigned to.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::json;

use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, one_of_or, required_int, text_list};
use crate::models::body::Fields;
use crate::models::shapes::{expect_body, names_of};

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_assign",
        description: "Add, remove or replace the assignees of an issue (or pull request) in a GitHub repository. GitHub ignores people who cannot be assigned, such as non-collaborators; the result lists any it did not apply.",
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
    "assignees": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "GitHub logins. At most 10 when adding."
    },
    "action": {
      "description": "add (default) keeps the current assignees and adds these; remove takes these off; set replaces all.",
      "type": "string",
      "enum": [
        "add",
        "remove",
        "set"
      ]
    }
  },
  "required": [
    "repo",
    "number",
    "assignees"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "assignees": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "not_applied": {
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "required": [
    "number",
    "assignees"
  ],
  "additionalProperties": false
}"#;

const ACTIONS: &[&str] = &["add", "remove", "set"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let assignees = text_list(arg(args, "assignees"), "assignees", true, 10)?.unwrap_or_default();
    let action = one_of_or(arg(args, "action"), "action", ACTIONS, "add")?;
    let path = format!("{}/issues/{number}", repo.path);
    let what = format!("{action} assignees on #{number} of {}", repo.full);

    let (method, target) = match action.as_str() {
        "add" => ("POST", format!("{path}/assignees")),
        "remove" => ("DELETE", format!("{path}/assignees")),
        _ => ("PATCH", path.clone()),
    };
    let answer = github(
        ctx,
        method,
        &target,
        GitHubOptions::what(what).json(json!({"assignees": assignees})),
    )
    .await?;
    let body = expect_body(&answer.body, "html_url", "an issue")?;

    let now = names_of(crate::models::body::get(body, "assignees"));
    let removing = action == "remove";
    let not_applied: Vec<String> = assignees
        .iter()
        .filter(|login| now.contains(login) == removing)
        .cloned()
        .collect();

    let listed = if now.is_empty() {
        "none".to_string()
    } else {
        now.join(", ")
    };
    let note = if not_applied.is_empty() {
        String::new()
    } else {
        let verb = if removing { "remove" } else { "assign" };
        let why = if removing {
            ""
        } else {
            " They may not be collaborators, or the token may lack push access."
        };
        format!(" GitHub did not {verb}: {}.{why}", not_applied.join(", "))
    };
    Outcome::ok(
        format!("Assignees of #{number} in {}: {listed}.{note}", repo.full),
        Fields::new()
            .set("number", number)
            .set("assignees", now)
            .opt(
                "not_applied",
                (!not_applied.is_empty()).then_some(not_applied),
            ),
    )
}

handler!(run);
