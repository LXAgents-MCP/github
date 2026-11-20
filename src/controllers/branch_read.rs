//! Read one branch.
use crate::handler;
use crate::models::address::{parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, at, boolean, get, nn, s, truthy};
use crate::models::schemas::{READ, schema};
use crate::models::shapes::expect_body;
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use crate::utils::text::{first_line, short};

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_read",
        description: "Read one branch of a GitHub repository: the commit at its tip, that commit's message and date, and whether the branch is protected. Use it to find a branch's current sha.",
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
    "branch": {
      "type": "string",
      "description": "Branch name, e.g. 'main' or 'feat/login'."
    }
  },
  "required": [
    "repo",
    "branch"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
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
    },
    "message": {
      "type": "string"
    },
    "date": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "sha",
    "protected"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let branch = ref_name(arg(args, "branch"), "branch")?;

    let answer = github(
        ctx,
        "GET",
        &format!("{}/branches/{}", repo.path, ref_path(&branch)),
        GitHubOptions::what(format!("read the branch '{branch}' of {}", repo.full)),
    )
    .await?;
    let body = expect_body(&answer.body, "name", "a branch")?;

    let sha = nn(at(body, "commit.sha"), "");
    let message = first_line(at(body, "commit.commit.message"));
    let date = at(body, "commit.commit.author.date");
    let protected = if truthy(get(body, "protected")) {
        " and is protected"
    } else {
        ""
    };
    Outcome::ok(
        format!(
            "Branch {} of {} is at {}{protected}.",
            s(get(body, "name")),
            repo.full,
            short(&sha)
        ),
        Fields::new()
            .set("name", get(body, "name").clone())
            .set("sha", sha)
            .set("protected", boolean(get(body, "protected")))
            .opt("message", (!message.is_empty()).then_some(message))
            .opt("date", crate::models::body::present(date)),
    )
}

handler!(run);
