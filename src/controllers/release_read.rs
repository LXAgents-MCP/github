//! Read one release.
use crate::handler;
use crate::models::address::{optional_ref, parse_repo};
use crate::models::args::{Args, arg};
use crate::models::schemas::{READ, schema};
use crate::models::shapes::{describe_release, expect_body, release_fields};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::services::releases::find_release_by_tag;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_read",
        description: "Read one release of a GitHub repository: title, notes, whether it is a draft or prerelease, and its assets. Give a tag, or omit it for the latest published release. Finds drafts by tag too.",
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
      "description": "Tag of the release. Omit for the latest published, non-prerelease one.",
      "type": "string"
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
    "id": {
      "type": "number"
    },
    "tag": {
      "type": "string"
    },
    "name": {
      "type": "string"
    },
    "draft": {
      "type": "boolean"
    },
    "prerelease": {
      "type": "boolean"
    },
    "html_url": {
      "type": "string"
    },
    "published_at": {
      "type": "string"
    },
    "body": {
      "type": "string"
    },
    "assets": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string"
          },
          "size": {
            "type": "number"
          },
          "downloads": {
            "type": "number"
          }
        },
        "required": [
          "name",
          "size",
          "downloads"
        ],
        "additionalProperties": false
      }
    }
  },
  "required": [
    "id",
    "tag",
    "draft",
    "prerelease",
    "html_url",
    "assets"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let tag = optional_ref(arg(args, "tag"), "tag")?;

    let release = match &tag {
        None => {
            let none = |status: u16, _: &str| {
                (status == 404).then(|| {
                    format!(
                        "{} has no published release yet. List what exists with release_list.",
                        repo.full
                    )
                })
            };
            github(
                ctx,
                "GET",
                &format!("{}/releases/latest", repo.path),
                GitHubOptions::what(format!("read the latest release of {}", repo.full))
                    .explain(&none),
            )
            .await?
            .body
        }
        Some(tag) => find_release_by_tag(ctx, &repo, tag).await?,
    };

    let fields = release_fields(expect_body(&release, "html_url", "a release")?);
    Outcome::ok(
        format!("{} has {}.", repo.full, describe_release(&fields)),
        fields,
    )
}

handler!(run);
