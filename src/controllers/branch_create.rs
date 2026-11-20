//! Create a branch.
use crate::handler;
use crate::models::schemas::{WRITE, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::models::address::{optional_ref, parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::Fields;
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_create",
        description: "Create a branch in a GitHub repository, pointing at a branch, tag or commit. Use it to start work off the default branch or off another branch. Fails if the name is taken.",
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
    "branch": {
      "type": "string",
      "description": "Name of the new branch, e.g. 'feat/login'."
    },
    "from": {
      "description": "Branch, tag or commit sha to start from. Default: the repository's default branch.",
      "type": "string"
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
    "branch": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "from": {
      "type": "string"
    }
  },
  "required": [
    "branch",
    "sha"
  ],
  "additionalProperties": false
}"#;

static SHA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{40,64}$").unwrap());

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let branch = ref_name(arg(args, "branch"), "branch")?;
    let from = optional_ref(arg(args, "from"), "from")?;

    // The sha media type answers with the commit's sha and nothing else, where the default answer is the
    // whole commit with its file list - far more than the one fact needed here.
    let start_ref = from
        .as_deref()
        .map(ref_path)
        .unwrap_or_else(|| "HEAD".to_string());
    let finding = match &from {
        None => "the default branch".to_string(),
        Some(from) => format!("'{from}'"),
    };
    let empty = |status: u16, _: &str| {
        (status == 409).then(|| {
            format!("{} is empty, so there is nothing to branch from. Create its first commit with file_create.", repo.full)
        })
    };
    let start = github(
        ctx,
        "GET",
        &format!("{}/commits/{start_ref}", repo.path),
        GitHubOptions::what(format!("find {finding} in {} to branch from", repo.full))
            .accept("application/vnd.github.sha")
            .raw()
            .explain(&empty),
    )
    .await?;
    let sha = start.body.as_str().map(str::trim).unwrap_or("").to_string();
    if !SHA.is_match(&sha) {
        return Err(Refused(
            "GitHub answered, but the body was not a commit sha this server recognizes."
                .to_string(),
        ));
    }

    let taken = |status: u16, detail: &str| {
        (status == 422 && detail.to_lowercase().contains("already exists")).then(|| {
            format!(
                "GitHub could not create the branch '{branch}' (422){detail}. A branch with that name already exists in {}; pick another name.",
                repo.full
            )
        })
    };
    github(
        ctx,
        "POST",
        &format!("{}/git/refs", repo.path),
        GitHubOptions::what(format!("create the branch '{branch}' in {}", repo.full))
            .json(json!({"ref": format!("refs/heads/{branch}"), "sha": sha}))
            .explain(&taken),
    )
    .await?;

    let from_text = from
        .as_ref()
        .map(|from| format!(", from {from}"))
        .unwrap_or_default();
    Outcome::ok(
        format!(
            "Created branch {branch} at {} in {}{from_text}.",
            short(&json!(sha)),
            repo.full
        ),
        Fields::new()
            .set("branch", branch.clone())
            .set("sha", sha.clone())
            .opt("from", from.clone()),
    )
}

handler!(run);
