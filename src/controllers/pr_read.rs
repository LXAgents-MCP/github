//! Read one pull request.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::config::config::MAX_FILES_SHOWN;
use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, optional_flag, required_int};
use crate::models::body::{Fields, at, boolean, count, file_rows, get, list, nn, s, text, truthy};
use crate::models::shapes::{expect_body, names_of, reviewers_of};

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_read",
        description: "Read one pull request of a GitHub repository: state, draft, merged, whether it can be merged, branches, size, labels, requested reviewers and description, and with `files: true` the first 100 changed files. Use issue_read with `comments: true` for the discussion.",
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Pull request number."
    },
    "files": {
      "description": "true also lists the changed files, without their diffs. Default false.",
      "type": "boolean"
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
    "title": {
      "type": "string"
    },
    "state": {
      "type": "string"
    },
    "draft": {
      "type": "boolean"
    },
    "merged": {
      "type": "boolean"
    },
    "mergeable": {
      "type": "boolean"
    },
    "mergeable_state": {
      "type": "string"
    },
    "author": {
      "type": "string"
    },
    "head": {
      "type": "string"
    },
    "base": {
      "type": "string"
    },
    "head_sha": {
      "type": "string"
    },
    "commits": {
      "type": "number"
    },
    "changed_files": {
      "type": "number"
    },
    "additions": {
      "type": "number"
    },
    "deletions": {
      "type": "number"
    },
    "labels": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "reviewers": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "html_url": {
      "type": "string"
    },
    "body": {
      "type": "string"
    },
    "files": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "filename": {
            "type": "string"
          },
          "status": {
            "type": "string"
          },
          "additions": {
            "type": "number"
          },
          "deletions": {
            "type": "number"
          }
        },
        "required": [
          "filename",
          "status",
          "additions",
          "deletions"
        ],
        "additionalProperties": false
      }
    }
  },
  "required": [
    "number",
    "title",
    "state",
    "draft",
    "merged",
    "head",
    "base",
    "head_sha",
    "commits",
    "changed_files",
    "additions",
    "deletions",
    "labels",
    "reviewers",
    "html_url"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let with_files = optional_flag(arg(args, "files"), "files", false)?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/pulls/{number}", repo.path),
        GitHubOptions::what(format!("read pull request #{number} of {}", repo.full)),
    )
    .await?;
    let body = expect_body(&answer.body, "html_url", "a pull request")?;

    let mut files: Option<Vec<Value>> = None;
    if with_files {
        let answer = github(
            ctx,
            "GET",
            &format!("{}/pulls/{number}/files", repo.path),
            GitHubOptions::what(format!(
                "list the files of pull request #{number} of {}",
                repo.full
            ))
            .query("per_page", MAX_FILES_SHOWN),
        )
        .await?;
        let listed = list(&answer.body);
        files = Some(file_rows(listed, listed.len()));
    }

    let state = if truthy(get(body, "merged")) {
        "merged".to_string()
    } else {
        s(get(body, "state"))
    };
    let head = nn(at(body, "head.ref"), "");
    let base = nn(at(body, "base.ref"), "");
    // `mergeable` is null while GitHub works it out in the background; that is "not known yet", not "no".
    let unknown = get(body, "mergeable").is_null();
    let shown_state = if truthy(get(body, "draft")) && state == "open" {
        "a draft, open".to_string()
    } else {
        state.clone()
    };
    let pending = if state == "open" && unknown {
        " Whether it can be merged is still being computed; ask again in a moment."
    } else {
        ""
    };
    Outcome::ok(
        format!(
            "Pull request #{number} in {} is {shown_state}: {} ({} into {}).{pending}",
            repo.full,
            s(get(body, "title")),
            s(&head),
            s(&base)
        ),
        Fields::new()
            .set("number", number)
            .set("title", get(body, "title").clone())
            .set("state", get(body, "state").clone())
            .set("draft", boolean(get(body, "draft")))
            .set("merged", boolean(get(body, "merged")))
            .opt(
                "mergeable",
                (!unknown).then(|| boolean(get(body, "mergeable"))),
            )
            .opt("mergeable_state", text(get(body, "mergeable_state")))
            .opt("author", text(at(body, "user.login")))
            .set("head", head)
            .set("base", base)
            .set("head_sha", nn(at(body, "head.sha"), ""))
            .set("commits", count(get(body, "commits")))
            .set("changed_files", count(get(body, "changed_files")))
            .set("additions", count(get(body, "additions")))
            .set("deletions", count(get(body, "deletions")))
            .set("labels", names_of(get(body, "labels")))
            .set("reviewers", reviewers_of(body))
            .set("html_url", get(body, "html_url").clone())
            .opt("body", text(get(body, "body")))
            .opt("files", files),
    )
}

handler!(run);
