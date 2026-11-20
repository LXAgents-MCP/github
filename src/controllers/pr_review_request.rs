//! Ask people or teams to review a pull request.
use crate::handler;
use crate::models::address::parse_repo;
use crate::models::args::{Args, Range, arg, required_int, text_list};
use crate::models::body::{Fields, get, list, s};
use crate::models::schemas::{UPDATE, schema};
use crate::models::shapes::{expect_body, names_of};
use crate::models::tool::Outcome;
use crate::models::tool::ToolDef;
use crate::services::api::{GitHubOptions, github};
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_review_request",
        description: "Request a review of a pull request in a GitHub repository from people, teams, or both. Reviewers must be collaborators and cannot be the pull request's author. Give at least one.",
        input_schema: schema(INPUT),
        output_schema: schema(OUTPUT),
        annotations: UPDATE,
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
    "number": {
      "type": "integer",
      "minimum": -9007199254740991,
      "maximum": 9007199254740991,
      "description": "Pull request number."
    },
    "reviewers": {
      "description": "GitHub logins to ask.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "team_reviewers": {
      "description": "Team slugs to ask, e.g. 'platform'.",
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "required": [
    "repo",
    "number"
  ]
}"#;

const OUTPUT: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "number": {
      "type": "number"
    },
    "reviewers": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "teams": {
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "required": [
    "number",
    "reviewers",
    "teams"
  ],
  "additionalProperties": false
}"#;

async fn run(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"), "repo")?;
    let number = required_int(arg(args, "number"), "number", Range::min(1))?;
    let reviewers =
        text_list(arg(args, "reviewers"), "reviewers", false, 100)?.filter(|list| !list.is_empty());
    let teams = text_list(arg(args, "team_reviewers"), "team_reviewers", false, 100)?
        .filter(|list| !list.is_empty());
    if reviewers.is_none() && teams.is_none() {
        return Err(Refused(
            "Give at least one of `reviewers` and `team_reviewers`.".to_string(),
        ));
    }

    let explain = |status: u16, detail: &str| {
        (status == 422).then(|| {
            format!(
                "GitHub would not request that review on #{number} of {} (422){detail}. Reviewers must be collaborators with read access, and the pull request's author cannot review their own work.",
                repo.full
            )
        })
    };
    let json = Fields::new()
        .opt("reviewers", reviewers)
        .opt("team_reviewers", teams)
        .into_value();
    let answer = github(
        ctx,
        "POST",
        &format!("{}/pulls/{number}/requested_reviewers", repo.path),
        GitHubOptions::what(format!(
            "request a review of pull request #{number} of {}",
            repo.full
        ))
        .json(json)
        .explain(&explain),
    )
    .await?;
    let body = expect_body(&answer.body, "html_url", "a pull request")?;

    let people = names_of(get(body, "requested_reviewers"));
    let slugs: Vec<serde_json::Value> = list(get(body, "requested_teams"))
        .iter()
        .map(|team| {
            let slug = get(team, "slug");
            if slug.is_null() {
                get(team, "name").clone()
            } else {
                slug.clone()
            }
        })
        .collect();
    let mut asked: Vec<String> = people.clone();
    asked.extend(slugs.iter().map(|slug| {
        format!(
            "team {}",
            if slug.is_null() {
                "undefined".to_string()
            } else {
                s(slug)
            }
        )
    }));
    let listed = if asked.is_empty() {
        "nobody".to_string()
    } else {
        asked.join(", ")
    };
    Outcome::ok(
        format!(
            "Review requested on #{number} in {} from: {listed}.",
            repo.full
        ),
        Fields::new()
            .set("number", number)
            .set("reviewers", people)
            .set("teams", slugs),
    )
}

handler!(run);
