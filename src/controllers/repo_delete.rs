//! Delete a repository.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, arg, required_text};
use crate::models::body::Fields;
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_delete",
        description: "Permanently delete a GitHub repository, with its issues, pull requests and wiki. Destructive and not undoable from here: confirm with the owner first. `confirm` must repeat the repository's full name. Needs delete rights on the token and admin on the repository.",
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
    "confirm": {
      "type": "string",
      "description": "The repository's full name again, 'owner/name'. Anything else deletes nothing."
    }
  },
  "required": [
    "repo",
    "confirm"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "deleted": {
      "type": "boolean"
    },
    "full_name": {
      "type": "string"
    }
  },
  "required": [
    "deleted",
    "full_name"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let confirm = required_text(arg(args, "confirm"), "confirm")?;

    // The guard is mechanical on purpose: a name typed back cannot be a reflex, a copied flag can.
    if confirm.to_lowercase() != repo.full.to_lowercase() {
        return Err(Refused(format!(
            "`confirm` must repeat the repository's full name, '{}', to delete it. Nothing was deleted.",
            repo.full
        )));
    }

    let explain = |status: u16, detail: &str| {
        (status == 403).then(|| {
            format!(
                "GitHub refused to delete {} (403){detail}. Deleting needs admin rights on the repository, and a token that may delete: the `delete_repo` scope for a classic token, the Administration permission set to write for a fine-grained one. An organization can also forbid its members to delete repositories.",
                repo.full
            )
        })
    };
    github(
        ctx,
        "DELETE",
        &repo.path,
        GitHubOptions::what(format!("delete the repository {}", repo.full)).explain(&explain),
    )
    .await?;

    Outcome::ok(
        format!("Deleted repository {}.", repo.full),
        Fields::new()
            .set("deleted", true)
            .set("full_name", repo.full.clone()),
    )
}

handler!(run);
