//! GitHub's objects as this server reports them: the function that fills each shape, and the sentence
//! about it. Kept apart from the tools so every tool that returns a repository (or a release, an issue, a
//! job) says the same things about it, and a field GitHub renames is changed once. The shapes' schemas
//! are in each tool's published output schema.
use serde_json::{Map, Value};

use super::body::{Fields, at, boolean, count, get, list, text, truthy};
use crate::utils::errors::Refused;

/// Check that an answer is the kind of object the tool expected, before reading fields off it.
///
/// GitHub answers 2xx with a body of the shape documented, and a proxy or an outage can answer 2xx with
/// something else. This turns that into a refusal that says what was wrong.
pub fn expect_body<'a>(body: &'a Value, field: &str, what: &str) -> Result<&'a Value, Refused> {
    if !body.is_object() || !get(body, field).is_string() {
        return Err(Refused(format!(
            "GitHub answered, but the body was not {what} this server recognizes."
        )));
    }
    Ok(body)
}

/// What every tool that returns a repository reports about it.
pub fn repository_fields(repo: &Value) -> Map<String, Value> {
    let visibility = match get(repo, "visibility") {
        Value::Null => Value::from(if truthy(get(repo, "private")) {
            "private"
        } else {
            "public"
        }),
        other => other.clone(),
    };
    let topics = match get(repo, "topics") {
        Value::Array(items) => Value::Array(items.clone()),
        _ => Value::Array(vec![]),
    };
    Fields::new()
        .set("full_name", get(repo, "full_name").clone())
        .opt("description", text(get(repo, "description")))
        .set("html_url", get(repo, "html_url").clone())
        .set("clone_url", get(repo, "clone_url").clone())
        .set("visibility", visibility)
        .opt("default_branch", get(repo, "default_branch").as_str())
        .set("fork", boolean(get(repo, "fork")))
        .set("archived", boolean(get(repo, "archived")))
        .set("stars", count(get(repo, "stargazers_count")))
        .set("forks", count(get(repo, "forks_count")))
        .set("open_issues", count(get(repo, "open_issues_count")))
        .opt("language", text(get(repo, "language")))
        .opt("license", text(at(repo, "license.spdx_id")))
        .set("topics", topics)
        .opt("pushed_at", text(get(repo, "pushed_at")))
        .opt("parent", text(at(repo, "parent.full_name")))
        .into_map()
}

/// One line about a repository, for a summary.
pub fn describe_repository(fields: &Map<String, Value>) -> String {
    let kind = if truthy(&fields["fork"]) {
        "fork"
    } else {
        "repository"
    };
    let archived = if truthy(&fields["archived"]) {
        "archived "
    } else {
        ""
    };
    let branch = fields
        .get("default_branch")
        .and_then(Value::as_str)
        .map(|b| format!(", default branch {b}"))
        .unwrap_or_default();
    format!(
        "{} is a {} {archived}{kind}{branch}.",
        crate::utils::render::js_string(&fields["full_name"]),
        crate::utils::render::js_string(&fields["visibility"])
    )
}

/// What every tool that returns a release reports about it.
pub fn release_fields(release: &Value) -> Map<String, Value> {
    let assets: Vec<Value> = list(get(release, "assets"))
        .iter()
        .map(|asset| {
            Fields::new()
                .set("name", get(asset, "name").clone())
                .set("size", count(get(asset, "size")))
                .set("downloads", count(get(asset, "download_count")))
                .into_value()
        })
        .collect();
    Fields::new()
        .set("id", get(release, "id").clone())
        .set("tag", get(release, "tag_name").clone())
        .opt("name", text(get(release, "name")))
        .set("draft", boolean(get(release, "draft")))
        .set("prerelease", boolean(get(release, "prerelease")))
        .set("html_url", get(release, "html_url").clone())
        .opt("published_at", text(get(release, "published_at")))
        .opt("body", text(get(release, "body")))
        .set("assets", assets)
        .into_map()
}

/// One line about a release, for a summary.
pub fn describe_release(fields: &Map<String, Value>) -> String {
    let kind = if truthy(&fields["draft"]) {
        "draft release"
    } else if truthy(&fields["prerelease"]) {
        "prerelease"
    } else {
        "release"
    };
    let tag = crate::utils::render::js_string(&fields["tag"]);
    let name = fields
        .get("name")
        .map(crate::utils::render::js_string)
        .unwrap_or_else(|| tag.clone());
    format!("{kind} {name} (tag {tag})")
}

/// The names in a list GitHub returns as strings or as objects with a `name` or a `login`.
pub fn names_of(value: &Value) -> Vec<String> {
    list(value)
        .iter()
        .filter_map(|entry| {
            let name = match entry {
                Value::String(_) => entry.clone(),
                _ => {
                    let name = get(entry, "name");
                    if name.is_null() {
                        get(entry, "login").clone()
                    } else {
                        name.clone()
                    }
                }
            };
            if truthy(&name) {
                Some(crate::utils::render::js_string(&name))
            } else {
                None
            }
        })
        .collect()
}

/// What `issue_read` reports about an issue - and, because a pull request is an issue, about that.
pub fn issue_fields(issue: &Value) -> Map<String, Value> {
    Fields::new()
        .set("number", get(issue, "number").clone())
        .set("title", get(issue, "title").clone())
        .set("state", get(issue, "state").clone())
        .opt("state_reason", text(get(issue, "state_reason")))
        .opt("author", text(at(issue, "user.login")))
        .set("labels", names_of(get(issue, "labels")))
        .set("assignees", names_of(get(issue, "assignees")))
        .opt("milestone", text(at(issue, "milestone.title")))
        .set("comment_count", count(get(issue, "comments")))
        .set("is_pull_request", boolean(get(issue, "pull_request")))
        .set("html_url", get(issue, "html_url").clone())
        .opt("created_at", text(get(issue, "created_at")))
        .opt("updated_at", text(get(issue, "updated_at")))
        .opt("closed_at", text(get(issue, "closed_at")))
        .opt("body", text(get(issue, "body")))
        .into_map()
}

/// The people and teams a review has been requested from, as one list; a team is `team:slug`.
pub fn reviewers_of(pull: &Value) -> Vec<String> {
    let mut names = names_of(get(pull, "requested_reviewers"));
    for team in list(get(pull, "requested_teams")) {
        let slug = get(team, "slug");
        let slug = if slug.is_null() {
            get(team, "name")
        } else {
            slug
        };
        let slug = if slug.is_null() {
            "undefined".to_string()
        } else {
            crate::utils::render::js_string(slug)
        };
        names.push(format!("team:{slug}"));
    }
    names
}

/// What the workflow tools report about a job of a run. `failed_step` is the first step that failed,
/// which is where to look in the job's log.
pub fn job_fields(job: &Value) -> Map<String, Value> {
    let failed = list(get(job, "steps")).iter().find(|step| {
        let conclusion = get(step, "conclusion").as_str();
        conclusion == Some("failure") || conclusion == Some("timed_out")
    });
    let name = match get(job, "name") {
        Value::Null => Value::from(""),
        other => other.clone(),
    };
    let status = match get(job, "status") {
        Value::Null => Value::from("unknown"),
        other => other.clone(),
    };
    Fields::new()
        .set("id", get(job, "id").clone())
        .set("name", name)
        .set("status", status)
        .opt("conclusion", text(get(job, "conclusion")))
        .opt(
            "failed_step",
            failed.and_then(|step| text(get(step, "name"))),
        )
        .into_map()
}
