//! Merge one branch into another, without a pull request.
use crate::handler;
use crate::models::address::{parse_repo, ref_name};
use crate::models::args::{Args, arg, optional_text};
use crate::models::body::{Fields, at, get};
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use crate::utils::text::{first_line, short};

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_merge",
        description: "Merge a branch (or commit) into another branch of a GitHub repository directly, with no pull request. Destructive: it moves the base branch, so confirm with the owner first. Use pr_merge when a pull request exists.",
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
    "base": {
      "type": "string",
      "description": "Branch that receives the merge, e.g. 'main'."
    },
    "head": {
      "type": "string",
      "description": "Branch or commit sha to merge into the base."
    },
    "message": {
      "description": "Commit message for the merge commit. Default: GitHub's.",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "base",
    "head"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "merged": {
      "type": "boolean"
    },
    "up_to_date": {
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
    "merged",
    "up_to_date"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let base = ref_name(arg(args, "base"), "base")?;
    let head = ref_name(arg(args, "head"), "head")?;
    let message = optional_text(arg(args, "message"), "message")?;

    let explain = |code: u16, detail: &str| match code {
        409 => Some(format!(
            "GitHub could not merge '{head}' into '{base}' in {} (409){detail}. The branches conflict. Open a pull request to resolve it there, or merge '{base}' into '{head}' first.",
            repo.full
        )),
        404 => Some(format!(
            "GitHub could not find '{base}' or '{head}' in {} (404){detail}. Check both names, and that the token can see the repository.",
            repo.full
        )),
        _ => None,
    };
    let json = Fields::new()
        .set("base", base.clone())
        .set("head", head.clone())
        .opt("commit_message", message)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!("{}/merges", repo.path),
        GitHubOptions::what(format!("merge '{head}' into '{base}' in {}", repo.full))
            .json(json)
            .explain(&explain),
    )
    .await?;

    // 204 is GitHub saying the base already contains the head: nothing was merged, and that is fine.
    if answer.status == 204 {
        return Outcome::ok(
            format!("Nothing to merge: {base} already contains {head}."),
            Fields::new().set("merged", false).set("up_to_date", true),
        );
    }
    let sha = get(&answer.body, "sha")
        .as_str()
        .filter(|sha| !sha.is_empty())
        .map(str::to_string);
    let text = first_line(at(&answer.body, "commit.message"));
    let at_sha = sha
        .as_ref()
        .map(|sha| format!(", at {}", short(&serde_json::json!(sha))))
        .unwrap_or_default();
    Outcome::ok(
        format!("Merged {head} into {base} in {}{at_sha}.", repo.full),
        Fields::new()
            .set("merged", true)
            .set("up_to_date", false)
            .opt("sha", sha)
            .opt("message", (!text.is_empty()).then_some(text)),
    )
}

handler!(run);
