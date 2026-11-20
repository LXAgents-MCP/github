//! Create a release.
use crate::handler;
use crate::models::address::{optional_ref, parse_repo, ref_name};
use crate::models::args::{Args, arg, maybe_flag, optional_text, raw_text};
use crate::models::body::Fields;
use crate::models::schemas::{WRITE, schema};
use crate::models::shapes::{describe_release, expect_body, release_fields};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_create",
        description: "Create a release for a tag in a GitHub repository, creating the tag from a branch or commit if it does not exist. Can be a draft or prerelease, and can have GitHub write the notes. This is also how a tag is created.",
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
    "tag": {
      "type": "string",
      "description": "Tag for the release, e.g. 'v1.2.0'. Created if it does not exist."
    },
    "target": {
      "description": "Branch or commit sha to create the tag from. Default: the default branch. Ignored if the tag exists.",
      "type": "string"
    },
    "name": {
      "description": "Release title. Default: the tag.",
      "type": "string"
    },
    "body": {
      "description": "Release notes, in Markdown.",
      "type": "string"
    },
    "draft": {
      "description": "true creates an unpublished draft. Default false.",
      "type": "boolean"
    },
    "prerelease": {
      "description": "true marks it as a prerelease. Default false.",
      "type": "boolean"
    },
    "generate_notes": {
      "description": "true has GitHub write the notes from merged pull requests, added to any body given.",
      "type": "boolean"
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
    let tag = ref_name(arg(args, "tag"), "tag")?;
    let target = optional_ref(arg(args, "target"), "target")?;
    let name = optional_text(arg(args, "name"), "name")?;
    let body = raw_text(arg(args, "body"), "body")?;
    let draft = maybe_flag(arg(args, "draft"), "draft")?;
    let prerelease = maybe_flag(arg(args, "prerelease"), "prerelease")?;
    let generate = maybe_flag(arg(args, "generate_notes"), "generate_notes")?;

    let json = Fields::new()
        .set("tag_name", tag.clone())
        .opt("target_commitish", target)
        .opt("name", name)
        .opt("body", body)
        .opt("draft", draft)
        .opt("prerelease", prerelease)
        .opt("generate_release_notes", generate)
        .into_value();
    let explain = |status: u16, detail: &str| {
        (status == 422 && detail.to_lowercase().contains("already_exists")).then(|| {
            format!(
                "GitHub could not create a release for '{tag}' in {} (422){detail}. A release for that tag already exists; read it with release_read, or use another tag.",
                repo.full
            )
        })
    };
    let answer = github(
        ctx,
        "POST",
        &format!("{}/releases", repo.path),
        GitHubOptions::what(format!(
            "create a release for the tag '{tag}' in {}",
            repo.full
        ))
        .json(json)
        .explain(&explain),
    )
    .await?;

    let fields = release_fields(expect_body(&answer.body, "html_url", "a release")?);
    Outcome::ok(
        format!("Created {} in {}.", describe_release(&fields), repo.full),
        fields,
    )
}

handler!(run);
