//! Create a repository on GitHub.  Moved from the `cli` server, where it was `create_github_repo`. The arguments, the defaults and the wording of its refusals are unchanged.
use crate::models::schemas::{WRITE, schema};
use crate::models::tool::ToolDef;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::handler;
use crate::models::address::{REPO_NAME, encode_component};
use crate::models::args::{Args, arg, optional_flag, optional_text, required_text};
use crate::models::body::{Fields, get, truthy};
use crate::models::tool::Outcome;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_create",
        description: "Create a new, empty repository on GitHub and return its URLs. Use this when the owner asks to start a new GitHub repository, in their own account or in an organization. Private unless told otherwise. It only creates: it does not clone, push to, change or delete anything. The token is read from the GITHUB_API_KEY environment variable of the server, never from an argument.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: WRITE,
    }
}

const INPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Repository name: letters, digits, '-', '_' and '.', at most 100 characters. Example: 'my-new-repo'."
    },
    "org": {
      "description": "Organization to create it in. Omit to create it in the personal account that owns the token. Example: 'LXAgents-MCP'.",
      "type": "string"
    },
    "description": {
      "description": "One-line description shown on the repository.",
      "type": "string"
    },
    "private": {
      "description": "Default true. Pass false only when the owner asked for a public repository.",
      "type": "boolean"
    },
    "auto_init": {
      "description": "Default true: create an initial commit with a README, so the repository can be cloned straight away. Pass false for a truly empty repository.",
      "type": "boolean"
    }
  },
  "required": [
    "name"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "name": {
      "type": "string"
    },
    "full_name": {
      "type": "string"
    },
    "html_url": {
      "type": "string"
    },
    "clone_url": {
      "type": "string"
    },
    "visibility": {
      "type": "string"
    },
    "default_branch": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "full_name",
    "html_url",
    "clone_url",
    "visibility"
  ],
  "additionalProperties": false
}"#;

static ORG_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9-]{1,39}$").unwrap());

/// This tool's wording for the statuses it knows; every status but 401 gets its own.
fn explain(
    status: u16,
    detail: &str,
    name: &str,
    org: Option<&str>,
    place: &str,
) -> Option<String> {
    match status {
        401 => None,
        403 => Some(format!(
            "GitHub refused the request (403){detail}. The token must be allowed to create repositories {place}: a classic token needs the `repo` scope, a fine-grained token needs the Administration permission set to write. If the message mentions a rate limit, wait and try again."
        )),
        404 => Some(match org {
            Some(org) => format!(
                "GitHub could not find the organization '{org}' (404), or the token cannot see it. Check the spelling, or omit `org` to create the repository in the token owner's account."
            ),
            None => format!("GitHub answered 404{detail}."),
        }),
        422 => Some(format!(
            "GitHub could not create '{name}' (422){detail}. The most common cause is that a repository with that name already exists {place}; pick another name."
        )),
        _ => Some(format!("GitHub answered {status}{detail}.")),
    }
}

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let name = required_text(arg(args, "name"), "name")?;
    if !REPO_NAME.is_match(&name) || name == "." || name == ".." {
        return Err(Refused(format!(
            "`name` '{name}' is not a valid GitHub repository name. Use letters, digits, '-', '_' and '.', at most 100 characters."
        )));
    }

    let org = optional_text(arg(args, "org"), "org")?;
    if let Some(org) = &org
        && !ORG_NAME.is_match(org)
    {
        return Err(Refused(format!(
            "`org` '{org}' is not a valid GitHub organization name. Use letters, digits and '-', at most 39 characters; omit it to create the repository in the token owner's account."
        )));
    }

    let description = optional_text(arg(args, "description"), "description")?;
    let is_private = optional_flag(arg(args, "private"), "private", true)?;
    let auto_init = optional_flag(arg(args, "auto_init"), "auto_init", true)?;

    let place = match &org {
        Some(org) => format!("in '{org}'"),
        None => "in the token owner's account".to_string(),
    };
    let path = match &org {
        Some(org) => format!("/orgs/{}/repos", encode_component(org)),
        None => "/user/repos".to_string(),
    };
    let json = Fields::new()
        .set("name", name.clone())
        .set("private", is_private)
        .set("auto_init", auto_init)
        .opt("description", description)
        .into_value();
    let explainer =
        |status: u16, detail: &str| explain(status, detail, &name, org.as_deref(), &place);
    let answer = github(
        ctx,
        "POST",
        &path,
        GitHubOptions::what(format!("create the repository '{name}' {place}"))
            .json(json)
            .explain(&explainer),
    )
    .await?;

    let body = &answer.body;
    if !get(body, "html_url").is_string() {
        return Err(Refused(format!(
            "GitHub answered {}, but the body was not a repository this server recognizes.",
            answer.status
        )));
    }

    let visibility = match get(body, "visibility") {
        serde_json::Value::Null => json!(if truthy(get(body, "private")) {
            "private"
        } else {
            "public"
        }),
        other => other.clone(),
    };
    let summary = format!(
        "Created {} repository {} on GitHub.",
        crate::utils::render::js_string(&visibility),
        crate::utils::render::js_string(get(body, "full_name"))
    );
    Outcome::ok(
        summary,
        Fields::new()
            .set("name", get(body, "name").clone())
            .set("full_name", get(body, "full_name").clone())
            .set("html_url", get(body, "html_url").clone())
            .set("clone_url", get(body, "clone_url").clone())
            .set("visibility", visibility)
            .opt("default_branch", get(body, "default_branch").as_str()),
    )
}

handler!(run);
