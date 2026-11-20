//! Read one tag, following an annotated tag to what it points at.
use crate::handler;
use crate::models::address::{encode_component, parse_repo, ref_name, ref_path};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, at, get, nn, present, text};
use crate::models::schemas::{READ, schema};
use crate::models::shapes::expect_body;
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use crate::utils::text::short;

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_get",
        description: "Read one tag of a GitHub repository: the commit it points at and, for an annotated tag, its message, tagger and date. Use it to find the commit behind a version tag. tag_check only says whether a tag exists.",
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
    "tag": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "commit_sha": {
      "type": "string"
    },
    "target_type": {
      "type": "string"
    },
    "annotated": {
      "type": "boolean"
    },
    "message": {
      "type": "string"
    },
    "tagger": {
      "type": "string"
    },
    "date": {
      "type": "string"
    }
  },
  "required": [
    "tag",
    "sha",
    "commit_sha",
    "target_type",
    "annotated"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    let missing = |status: u16, _: &str| {
        (status == 404).then(|| {
            format!(
                "There is no tag '{tag}' in {}. List them with tag_get_all.",
                repo.full
            )
        })
    };
    let reference = github(
        ctx,
        "GET",
        &format!("{}/git/ref/tags/{}", repo.path, ref_path(&tag)),
        GitHubOptions::what(format!("read the tag '{tag}' of {}", repo.full)).explain(&missing),
    )
    .await?;
    let object = get(&reference.body, "object");
    let Some(sha) = get(object, "sha").as_str().map(str::to_string) else {
        return Err(Refused(
            "GitHub answered, but the body was not a tag reference this server recognizes."
                .to_string(),
        ));
    };

    // A lightweight tag is only a reference to a commit. An annotated one is an object of its own, which
    // holds the message and the tagger and points at the commit.
    if get(object, "type").as_str() != Some("tag") {
        return Outcome::ok(
            format!(
                "Tag {tag} is a lightweight tag pointing at {}.",
                short(get(object, "sha"))
            ),
            Fields::new()
                .set("tag", tag.clone())
                .set("sha", sha.clone())
                .set("commit_sha", sha.clone())
                .set("target_type", nn(get(object, "type"), "commit"))
                .set("annotated", false),
        );
    }

    let answer = github(
        ctx,
        "GET",
        &format!("{}/git/tags/{}", repo.path, encode_component(&sha)),
        GitHubOptions::what(format!("read the annotated tag '{tag}' of {}", repo.full)),
    )
    .await?;
    let body = &answer.body;
    let target = expect_body(get(body, "object"), "sha", "an annotated tag")?;
    Outcome::ok(
        format!(
            "Tag {tag} is annotated and points at {}.",
            short(get(target, "sha"))
        ),
        Fields::new()
            .set("tag", tag.clone())
            .set("sha", sha)
            .set("commit_sha", get(target, "sha").clone())
            .set("target_type", nn(get(target, "type"), "commit"))
            .set("annotated", true)
            .opt("message", present(get(body, "message")))
            .opt("tagger", text(at(body, "tagger.name")))
            .opt("date", present(at(body, "tagger.date"))),
    )
}

handler!(run);
