//! Read one text file from a repository.
use crate::handler;
use crate::models::schemas::{READ, schema};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;
use std::sync::LazyLock;

use base64::Engine;
use regex::Regex;

use crate::config::config::MAX_FILE_CHARS;
use crate::models::address::{file_path, optional_ref, parse_repo};
use crate::models::args::{Args, arg};
use crate::models::body::{Fields, get, is_int, s, text};
use crate::services::contents::contents_path;
use crate::utils::text::{cut_text, decode_text, looks_binary};

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_read",
        description: "Read one text file from a GitHub repository, on the default branch or at a branch, tag or commit. Returns its content, size and blob sha. Cut at 25,000 characters; binary files and files over 1 MB are refused. Use directory_list to find a path first.",
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
    "path": {
      "type": "string",
      "description": "Path of the file inside the repository, e.g. 'src/index.js'."
    },
    "ref": {
      "description": "Branch, tag or commit sha. Default: the default branch.",
      "type": "string"
    }
  },
  "required": [
    "repo",
    "path"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "path": {
      "type": "string"
    },
    "sha": {
      "type": "string"
    },
    "size": {
      "type": "number"
    },
    "text": {
      "type": "string"
    },
    "truncated": {
      "type": "boolean"
    }
  },
  "required": [
    "path",
    "sha",
    "size",
    "text",
    "truncated"
  ],
  "additionalProperties": false
}"#;

static TOO_LARGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)too[_ ]large").unwrap());

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;

    let explain = |status: u16, detail: &str| {
        (status == 403 && TOO_LARGE.is_match(detail)).then(|| {
            format!(
                "'{}' in {} is over the 1 MB GitHub returns through this API (403){detail}. Fetch it with git instead.",
                file.path, repo.full
            )
        })
    };
    let answer = github(
        ctx,
        "GET",
        &contents_path(&repo, &file),
        GitHubOptions::what(format!("read '{}' in {}", file.path, repo.full))
            .query("ref", reference.clone())
            .explain(&explain),
    )
    .await?;
    let body = &answer.body;

    if body.is_array() {
        return Err(Refused(format!(
            "`path` '{}' is a directory in {}. Use directory_list to see inside it.",
            file.path, repo.full
        )));
    }
    match get(body, "type").as_str() {
        Some("symlink") => {
            let target = match get(body, "target") {
                serde_json::Value::Null => "another path".to_string(),
                target => s(target),
            };
            return Err(Refused(format!(
                "`path` '{}' is a symbolic link to '{target}'. Read that path instead.",
                file.path
            )));
        }
        Some("submodule") => {
            let of = text(get(body, "submodule_git_url"))
                .map(|url| format!(" of {url}"))
                .unwrap_or_default();
            return Err(Refused(format!(
                "`path` '{}' is a submodule{of}, not a file in this repository.",
                file.path
            )));
        }
        _ => {}
    }
    let content = match (
        get(body, "type").as_str(),
        get(body, "encoding").as_str(),
        get(body, "content").as_str(),
    ) {
        (Some("file"), Some("base64"), Some(content)) => content,
        _ => {
            return Err(Refused(
                "GitHub answered, but the body was not a file this server recognizes.".to_string(),
            ));
        }
    };

    // GitHub wraps base64 at 60 columns; Buffer.from ignored the line breaks, and so does this.
    let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(cleaned.trim_end_matches('='))
        .unwrap_or_default();
    if looks_binary(&bytes) {
        return Err(Refused(format!(
            "`path` '{}' is a binary file ({} bytes); this tool reads text.",
            file.path,
            bytes.len()
        )));
    }
    let (text, truncated) = cut_text(&decode_text(&bytes), MAX_FILE_CHARS);
    let size = if is_int(get(body, "size")) {
        get(body, "size").clone()
    } else {
        bytes.len().into()
    };

    let at = reference
        .as_ref()
        .map(|r| format!(" at {r}"))
        .unwrap_or_default();
    let cut = if truncated {
        format!(", cut at {MAX_FILE_CHARS} characters.")
    } else {
        ".".to_string()
    };
    Outcome::ok(
        format!(
            "Read {} ({} bytes) from {}{at}{cut}",
            file.path,
            s(&size),
            repo.full
        ),
        Fields::new()
            .set("path", file.path.clone())
            .set("sha", get(body, "sha").clone())
            .set("size", size)
            .set("text", text)
            .set("truncated", truncated),
    )
}

handler!(run);
