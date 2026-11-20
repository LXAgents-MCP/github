//! Delete a branch.
use crate::handler;
use crate::models::address::{parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::Fields;
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_delete",
        description: "Delete a branch from a GitHub repository. Destructive: commits only that branch holds become unreachable, so confirm with the owner first. The default branch cannot be deleted.",
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
    "branch": {
      "type": "string",
      "description": "Name of the branch to delete."
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
    "deleted": {
      "type": "boolean"
    },
    "branch": {
      "type": "string"
    }
  },
  "required": [
    "deleted",
    "branch"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let branch = ref_name(arg(args, "branch"), "branch")?;

    let explain = |status: u16, detail: &str| {
        (status == 422).then(|| {
            format!(
                "GitHub would not delete the branch '{branch}' from {} (422){detail}. The branch may not exist, or it is the default branch, which cannot be deleted until another branch is made the default.",
                repo.full
            )
        })
    };
    github(
        ctx,
        "DELETE",
        &format!("{}/git/refs/heads/{}", repo.path, ref_path(&branch)),
        GitHubOptions::what(format!("delete the branch '{branch}' from {}", repo.full))
            .explain(&explain),
    )
    .await?;

    Outcome::ok(
        format!("Deleted branch {branch} from {}.", repo.full),
        Fields::new()
            .set("deleted", true)
            .set("branch", branch.clone()),
    )
}

handler!(run);
