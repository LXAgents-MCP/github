//! Delete a tag.
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
        name: "tag_delete",
        description: "Delete a tag from a GitHub repository. Destructive: anything that pins the tag, such as a release, loses its anchor, so confirm with the owner first. Deleting a release does not delete its tag, and this does not delete the release.",
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
    "tag": {
      "type": "string",
      "description": "Name of the tag to delete."
    }
  },
  "required": [
    "repo",
    "tag"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "deleted": {
      "type": "boolean"
    },
    "tag": {
      "type": "string"
    }
  },
  "required": [
    "deleted",
    "tag"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    let explain = |status: u16, detail: &str| {
        (status == 422).then(|| {
            format!(
                "GitHub would not delete the tag '{tag}' from {} (422){detail}. It probably does not exist; list the tags with tag_get_all.",
                repo.full
            )
        })
    };
    github(
        ctx,
        "DELETE",
        &format!("{}/git/refs/tags/{}", repo.path, ref_path(&tag)),
        GitHubOptions::what(format!("delete the tag '{tag}' from {}", repo.full)).explain(&explain),
    )
    .await?;

    Outcome::ok(
        format!("Deleted tag {tag} from {}.", repo.full),
        Fields::new().set("deleted", true).set("tag", tag.clone()),
    )
}

handler!(run);
