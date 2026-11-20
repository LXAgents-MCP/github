//! Does a tag exist?
use crate::handler;
use crate::models::address::{parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, at, truthy};
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_check",
        description: "Check whether a tag exists in a GitHub repository, and if it does, what it points at and whether it is annotated. A missing tag is an answer, not an error. Use it before creating a release, or to avoid reusing a tag name.",
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
    "tag": {
      "type": "string",
      "description": "Tag name, e.g. 'v1.2.0'."
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
    "exists": {
      "type": "boolean"
    },
    "tag": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "annotated": {
      "type": "boolean"
    }
  },
  "required": [
    "exists",
    "tag"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    // A tag that is not there is the answer to this call, so a 404 is returned and not refused.
    let answer = github(
        ctx,
        "GET",
        &format!("{}/git/ref/tags/{}", repo.path, ref_path(&tag)),
        GitHubOptions::what(format!("look for the tag '{tag}' in {}", repo.full)).allow(&[404]),
    )
    .await?;
    let body = &answer.body;

    if answer.status == 404 || !truthy(at(body, "object.sha")) {
        return Outcome::ok(
            format!("Tag {tag} does not exist in {}.", repo.full),
            Fields::new().set("exists", false).set("tag", tag.clone()),
        );
    }
    let annotated = at(body, "object.type").as_str() == Some("tag");
    let kind = if annotated {
        "annotated"
    } else {
        "lightweight"
    };
    Outcome::ok(
        format!(
            "Tag {tag} exists in {} ({kind}) and points at {}.",
            repo.full,
            short(at(body, "object.sha"))
        ),
        Fields::new()
            .set("exists", true)
            .set("tag", tag.clone())
            .set("sha", at(body, "object.sha").clone())
            .set("annotated", annotated),
    )
}

handler!(run);
