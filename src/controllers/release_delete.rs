//! Delete a release, keeping its tag.
use crate::handler;
use crate::models::schemas::{DESTROY, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use serde_json::Value;

use crate::models::address::{encode_component, optional_ref, parse_repo};
use crate::models::args::{Args, Range, arg, optional_int};
use crate::models::body::{Fields, get, s};
use crate::services::releases::find_release_by_tag;

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_delete",
        description: "Delete a release of a GitHub repository, by tag or by id. Destructive: its notes and assets are lost, so confirm with the owner first. The tag is kept; use tag_delete to remove that too. Finds drafts by tag as well.",
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
      "description": "Tag of the release to delete. Give this or release_id.",
      "type": "string"
    },
    "release_id": {
      "description": "Id of the release (from release_list). Give this or tag.",
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991
    }
  },
  "required": [
    "repo"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "deleted": {
      "type": "boolean"
    },
    "id": {
      "type": "number"
    },
    "tag": {
      "type": "string"
    }
  },
  "required": [
    "deleted",
    "id"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let tag = optional_ref(arg(args, "tag"), "tag")?;
    let given = optional_int(arg(args, "release_id"), "release_id", Range::min(1))?;
    if tag.is_none() == given.is_none() {
        return Err(Refused(
            "Give exactly one of `tag` and `release_id`.".to_string(),
        ));
    }

    let id: Value = match (given, &tag) {
        (Some(id), _) => id.into(),
        (None, Some(tag)) => get(&find_release_by_tag(ctx, &repo, tag).await?, "id").clone(),
        (None, None) => unreachable!("one of the two was checked to be given"),
    };
    let which = match &tag {
        Some(tag) => format!("for '{tag}'"),
        None => s(&id),
    };
    github(
        ctx,
        "DELETE",
        &format!("{}/releases/{}", repo.path, encode_component(&s(&id))),
        GitHubOptions::what(format!("delete the release {which} of {}", repo.full)),
    )
    .await?;

    let which = match &tag {
        Some(tag) => format!("for {tag}"),
        None => s(&id),
    };
    Outcome::ok(
        format!(
            "Deleted the release {which} of {}. Its tag was kept.",
            repo.full
        ),
        Fields::new()
            .set("deleted", true)
            .set("id", id)
            .opt("tag", tag),
    )
}

handler!(run);
