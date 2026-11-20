//! Change the labels on an issue.
use crate::handler;
use crate::models::schemas::{UPDATE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::{Value, json};

use crate::models::address::{encode_component, parse_repo};
use crate::models::args::{Args, Range, arg, one_of_or, required_int, text_list};
use crate::models::body::Fields;
use crate::models::shapes::names_of;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_label",
        description: "Add, remove or replace the labels on an issue (or pull request) in a GitHub repository. The result lists the labels now on the issue and any requested label that is not as asked.",
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
    "labels": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Label names, e.g. ['bug', 'good first issue']."
    },
    "action": {
      "description": "add (default) keeps the current labels and adds these; remove takes these off; set replaces all.",
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
    "labels"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "labels": {
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
    "labels"
  ],
  "additionalProperties": false
}"#;

const ACTIONS: &[&str] = &["add", "remove", "set"];

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let labels = text_list(arg(args, "labels"), "labels", true, 100)?.unwrap_or_default();
    let action = one_of_or(arg(args, "action"), "action", ACTIONS, "add")?;
    let path = format!("{}/issues/{number}/labels", repo.path);
    let what = format!("{action} labels on #{number} of {}", repo.full);

    let (now, not_applied) = if action == "remove" {
        // One request per label: GitHub removes a single label by name. A 404 means that label is not on
        // the issue (or does not exist), which for "make sure it is off" is not a failure.
        let mut latest: Option<Value> = None;
        let mut not_applied = Vec::new();
        for label in &labels {
            let answer = github(
                ctx,
                "DELETE",
                &format!("{path}/{}", encode_component(label)),
                GitHubOptions::what(what.clone()).allow(&[404]),
            )
            .await?;
            if answer.status == 404 {
                not_applied.push(label.clone());
            } else {
                latest = Some(answer.body);
            }
        }
        let latest = match latest.filter(|body| !body.is_null()) {
            Some(body) => body,
            None => {
                github(
                    ctx,
                    "GET",
                    &path,
                    GitHubOptions::what(format!("read the labels on #{number} of {}", repo.full)),
                )
                .await?
                .body
            }
        };
        (names_of(&latest), not_applied)
    } else {
        let method = if action == "add" { "POST" } else { "PUT" };
        let answer = github(
            ctx,
            method,
            &path,
            GitHubOptions::what(what).json(json!({"labels": labels})),
        )
        .await?;
        let now = names_of(&answer.body);
        let not_applied = labels
            .iter()
            .filter(|label| !now.contains(label))
            .cloned()
            .collect();
        (now, not_applied)
    };

    let listed = if now.is_empty() {
        "none".to_string()
    } else {
        now.join(", ")
    };
    let note = if not_applied.is_empty() {
        String::new()
    } else {
        let lead = if action == "remove" {
            "Not on the issue to begin with"
        } else {
            "GitHub did not apply"
        };
        format!(" {lead}: {}.", not_applied.join(", "))
    };
    Outcome::ok(
        format!("Labels on #{number} in {}: {listed}.{note}", repo.full),
        Fields::new().set("number", number).set("labels", now).opt(
            "not_applied",
            (!not_applied.is_empty()).then_some(not_applied),
        ),
    )
}

handler!(run);
