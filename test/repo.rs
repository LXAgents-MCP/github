//! Ported from test/repo.test.ts.
use lxagents_github::config::config::TOKEN_ENV;
use lxagents_github::config::version::VERSION;
use lxagents_github::services::context::TransportError;
use serde_json::{Value, json};

use crate::helpers::*;

/// Merge `overrides` into `base`; a `null` override removes the key (TS `undefined`).
fn with(base: Value, overrides: Value) -> Value {
    let mut map = obj(base);
    for (k, v) in obj(overrides) {
        if v.is_null() {
            map.remove(&k);
        } else {
            map.insert(k, v);
        }
    }
    Value::Object(map)
}

/// Like `with`, but a `null` override stays as JSON null.
fn with_nulls(base: Value, overrides: Value) -> Value {
    let mut map = obj(base);
    map.extend(obj(overrides));
    Value::Object(map)
}

fn path_of(url: &str) -> &str {
    url.split('?').next().unwrap()
}

fn query(url: &str) -> Vec<(String, String)> {
    let Some((_, q)) = url.split_once('?') else {
        return vec![];
    };
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            let dec = |s: &str| {
                percent_encoding::percent_decode_str(&s.replace('+', " "))
                    .decode_utf8_lossy()
                    .into_owned()
            };
            (dec(k), dec(v))
        })
        .collect()
}

fn has_param(url: &str, name: &str) -> bool {
    query(url).iter().any(|(k, _)| k == name)
}

/* ------------------------------------------------------------------ repo_create */

fn created(overrides: Value) -> Value {
    with(
        json!({
            "name": "demo",
            "full_name": "me/demo",
            "html_url": "https://github.com/me/demo",
            "clone_url": "https://github.com/me/demo.git",
            "private": true,
            "visibility": "private",
            "default_branch": "main",
        }),
        overrides,
    )
}

#[test]
fn repo_create_the_token_variable_is_the_one_the_owner_named() {
    assert_eq!(TOKEN_ENV, "GITHUB_API_KEY");
    let d = def("repo_create");
    assert_eq!(d.name, "repo_create");
    assert!(d.description.contains(TOKEN_ENV));
    assert_eq!(
        input_keys("repo_create"),
        ["name", "org", "description", "private", "auto_init"]
    );
}

#[tokio::test]
async fn repo_create_created_in_the_token_owners_account_private_and_initialized_by_default() {
    let world = with_token(vec![reply(201, created(json!({})))]);
    let done = call(&world, "repo_create", json!({"name": "demo"}))
        .await
        .unwrap();
    assert!(done.ok);
    let payload = done.payload;
    assert_eq!(
        payload,
        json!({
            "summary": "Created private repository me/demo on GitHub.",
            "name": "demo",
            "full_name": "me/demo",
            "html_url": "https://github.com/me/demo",
            "clone_url": "https://github.com/me/demo.git",
            "visibility": "private",
            "default_branch": "main",
        })
    );
    expect_output(&def("repo_create"), &payload);
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    let c = &calls[0];
    assert_eq!(c.url, "https://api.github.com/user/repos");
    assert_eq!(c.method, "POST");
    assert_eq!(
        c.body,
        json!({"name": "demo", "private": true, "auto_init": true})
    );
    assert_eq!(
        c.header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(c.header("Accept"), Some("application/vnd.github+json"));
    assert_eq!(c.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    assert_eq!(
        c.header("User-Agent"),
        Some(format!("lxagents-mcp-github/{VERSION}").as_str())
    );
    assert_eq!(c.header("Content-Type"), Some("application/json"));
}

#[tokio::test]
async fn repo_create_an_organization_changes_the_endpoint_and_the_options_reach_github() {
    let world = with_token(vec![reply(
        201,
        created(json!({"full_name": "acme/demo", "private": false, "visibility": "public"})),
    )]);
    let payload = ok(
        &world,
        "repo_create",
        json!({"name": "demo", "org": "acme", "description": "  A demo  ", "private": false, "auto_init": false}),
    )
    .await;
    assert_eq!(
        payload["summary"],
        "Created public repository acme/demo on GitHub."
    );
    let calls = world.calls();
    assert_eq!(calls[0].url, "https://api.github.com/orgs/acme/repos");
    assert_eq!(
        calls[0].body,
        json!({"name": "demo", "private": false, "auto_init": false, "description": "A demo"})
    );
}

#[tokio::test]
async fn repo_create_a_blank_org_means_the_personal_account() {
    let world = with_token(vec![reply(201, created(json!({})))]);
    ok(&world, "repo_create", json!({"name": "demo", "org": "   "})).await;
    assert_eq!(world.calls()[0].url, "https://api.github.com/user/repos");
}

#[tokio::test]
async fn repo_create_visibility_falls_back_to_the_private_flag_and_a_missing_default_branch_is_left_out()
 {
    let world = with_token(vec![reply(
        201,
        created(json!({"visibility": null, "private": false, "default_branch": null})),
    )]);
    let payload = ok(
        &world,
        "repo_create",
        json!({"name": "demo", "private": false}),
    )
    .await;
    assert_eq!(payload["visibility"], "public");
    assert!(payload.get("default_branch").is_none());
}

#[tokio::test]
async fn repo_create_a_missing_token_is_a_refusal_that_names_the_variable_and_nothing_is_sent() {
    let world = without_token(vec![reply(201, created(json!({})))]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(&m, r"^GITHUB_API_KEY is not set\.");
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_create_bad_arguments_are_refused_before_the_token_is_read_or_anything_is_sent() {
    let world = without_token(vec![reply(201, created(json!({})))]);
    let cases = [
        (json!({}), "`name` is required"),
        (json!({"name": "   "}), "`name` is required"),
        (json!({"name": 5}), "`name` must be a string"),
        (
            json!({"name": "has space"}),
            "not a valid GitHub repository name",
        ),
        (json!({"name": "a/b"}), "not a valid GitHub repository name"),
        (json!({"name": ".."}), "not a valid GitHub repository name"),
        (
            json!({"name": "a".repeat(101)}),
            "not a valid GitHub repository name",
        ),
        (
            json!({"name": "ok", "org": "bad/org"}),
            "not a valid GitHub organization name",
        ),
        (
            json!({"name": "ok", "org": "-".repeat(40)}),
            "not a valid GitHub organization name",
        ),
        (
            json!({"name": "ok", "private": "no"}),
            "`private` must be true or false",
        ),
        (
            json!({"name": "ok", "auto_init": 1}),
            "`auto_init` must be true or false",
        ),
        (
            json!({"name": "ok", "description": 3}),
            "`description` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        let m = refused(&world, "repo_create", args).await;
        assert_matches(&m, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_create_401_says_the_token_was_rejected() {
    let world = with_token(vec![reply(401, json!({"message": "Bad credentials"}))]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(
        &m,
        r"^GitHub rejected the token \(401\)\. Check that GITHUB_API_KEY holds a valid token",
    );
}

#[tokio::test]
async fn repo_create_403_says_what_permission_the_token_needs() {
    let world = with_token(vec![
        reply(
            403,
            json!({"message": "Resource not accessible by personal access token"}),
        ),
        reply(403, json!({"message": "no"})),
    ]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(
        &m,
        r"403\): Resource not accessible .* in the token owner's account: a classic token needs the `repo` scope",
    );
    let m = refused(
        &world,
        "repo_create",
        json!({"name": "demo", "org": "acme"}),
    )
    .await;
    assert_matches(&m, "repositories in 'acme'");
}

#[tokio::test]
async fn repo_create_404_on_an_organization_says_the_organization_may_not_exist() {
    let world = with_token(vec![
        reply(404, json!({"message": "Not Found"})),
        reply(404, json!({"message": "Not Found"})),
    ]);
    let m = refused(
        &world,
        "repo_create",
        json!({"name": "demo", "org": "ghost"}),
    )
    .await;
    assert_matches(
        &m,
        r"could not find the organization 'ghost' \(404\).*omit `org`",
    );
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(&m, r"^GitHub answered 404: Not Found\.$");
}

#[tokio::test]
async fn repo_create_422_carries_githubs_reason() {
    let world = with_token(vec![reply(
        422,
        json!({
            "message": "Repository creation failed.",
            "errors": [{"resource": "Repository", "code": "custom", "field": "name", "message": "name already exists on this account"}],
        }),
    )]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(
        &m,
        r"could not create 'demo' \(422\): Repository creation failed\. \(name already exists on this account\)\..*already exists in the token owner's account",
    );
}

#[tokio::test]
async fn repo_create_any_other_status_is_reported_with_its_status_and_message() {
    let world = with_token(vec![reply(500, json!({"message": "Server Error"}))]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(&m, r"^GitHub answered 500: Server Error\.$");
}

#[tokio::test]
async fn repo_create_a_failure_never_carries_the_token() {
    let world = with_token(vec![reply(
        500,
        json!({"message": format!("bad token {TOKEN}")}),
    )]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert!(!m.contains(TOKEN) && m.contains("***"), "{m}");
}

#[tokio::test]
async fn repo_create_a_201_whose_body_is_not_a_repository_is_refused() {
    let world = with_token(vec![reply(201, json!({"unexpected": true}))]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(&m, "answered 201, but the body was not a repository");
}

#[tokio::test]
async fn repo_create_a_network_failure_is_a_refusal() {
    let world = with_token(vec![Answer::Fail(TransportError::Network(
        "ECONNREFUSED".into(),
    ))]);
    let m = refused(&world, "repo_create", json!({"name": "demo"})).await;
    assert_matches(&m, r"^Could not reach api\.github\.com: ECONNREFUSED\.");
}

/* ------------------------------------------------------------------ the other repository tools */

fn repository(overrides: Value) -> Value {
    with_nulls(
        json!({
            "full_name": "o/r",
            "description": "A thing",
            "html_url": "https://github.com/o/r",
            "clone_url": "https://github.com/o/r.git",
            "private": false,
            "visibility": "public",
            "default_branch": "main",
            "fork": false,
            "archived": false,
            "stargazers_count": 5,
            "forks_count": 2,
            "open_issues_count": 3,
            "language": "JavaScript",
            "license": {"spdx_id": "MIT"},
            "topics": ["mcp"],
            "pushed_at": "2026-10-01T00:00:00Z",
        }),
        overrides,
    )
}

fn facts(summary: &str) -> Value {
    json!({
        "summary": summary,
        "full_name": "o/r",
        "description": "A thing",
        "html_url": "https://github.com/o/r",
        "clone_url": "https://github.com/o/r.git",
        "visibility": "public",
        "default_branch": "main",
        "fork": false,
        "archived": false,
        "stars": 5,
        "forks": 2,
        "open_issues": 3,
        "language": "JavaScript",
        "license": "MIT",
        "topics": ["mcp"],
        "pushed_at": "2026-10-01T00:00:00Z",
    })
}

#[tokio::test]
async fn repo_read_reads_one_repository_and_reports_its_facts() {
    let world = with_token(vec![reply(200, repository(json!({})))]);
    let done = call(&world, "repo_read", json!({"repo": "o/r"}))
        .await
        .unwrap();
    assert!(done.ok);
    assert_eq!(
        done.payload,
        facts("o/r is a public repository, default branch main.")
    );
    expect_output(&def("repo_read"), &done.payload);
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "GET");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r");
}

#[tokio::test]
async fn repo_read_says_when_a_repository_is_a_private_archived_fork() {
    let mut body = repository(json!({
        "private": true, "fork": true, "archived": true,
        "description": null, "language": null, "license": null,
        "parent": {"full_name": "up/stream"},
    }));
    let map = body.as_object_mut().unwrap();
    for k in ["visibility", "topics", "pushed_at"] {
        map.remove(k);
    }
    let world = with_token(vec![reply(200, body)]);
    let payload = ok(&world, "repo_read", json!({"repo": "o/r"})).await;
    assert_eq!(
        payload["summary"],
        "o/r is a private archived fork, default branch main."
    );
    assert_eq!(payload["parent"], "up/stream");
    assert_eq!(payload["topics"], json!([]));
    for absent in ["description", "language", "license", "pushed_at"] {
        assert!(payload.get(absent).is_none(), "{absent}");
    }
}

#[tokio::test]
async fn repo_read_refuses_a_bad_repo_a_404_and_an_unexpected_body() {
    let world = with_token(vec![
        reply(404, json!({"message": "Not Found"})),
        reply(200, json!({"nope": true})),
    ]);
    assert_matches(
        &refused(&world, "repo_read", json!({"repo": "nope"})).await,
        "`repo` 'nope' is not a repository",
    );
    assert_matches(
        &refused(&world, "repo_read", json!({})).await,
        "`repo` is required",
    );
    assert_eq!(world.calls().len(), 0);
    assert_matches(
        &refused(&world, "repo_read", json!({"repo": "o/r"})).await,
        r"^GitHub could not find what it needs to read the repository o/r \(404\): Not Found\. .*private repository",
    );
    assert_matches(
        &refused(&world, "repo_read", json!({"repo": "o/r"})).await,
        "the body was not a repository this server recognizes",
    );
}

#[tokio::test]
async fn repo_update_sends_only_what_changes_and_says_what_it_changed() {
    let world = with_token(vec![reply(
        200,
        repository(json!({"description": "New", "archived": true})),
    )]);
    let payload = ok(
        &world,
        "repo_update",
        json!({"repo": "o/r", "description": "New", "archived": true, "private": false}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "PATCH");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r");
    assert_eq!(
        calls[0].body,
        json!({"description": "New", "private": false, "archived": true})
    );
    assert_eq!(
        payload["summary"],
        "Updated o/r: description, private, archived."
    );
    assert_eq!(payload["archived"], true);
}

#[tokio::test]
async fn repo_update_can_clear_a_description_and_a_homepage_and_rename() {
    let world = with_token(vec![reply(
        200,
        repository(json!({"full_name": "o/renamed", "description": null})),
    )]);
    let payload = ok(
        &world,
        "repo_update",
        json!({"repo": "o/r", "description": "", "homepage": "", "name": " renamed "}),
    )
    .await;
    assert_eq!(
        world.calls()[0].body,
        json!({"name": "renamed", "description": "", "homepage": ""})
    );
    assert_eq!(payload["full_name"], "o/renamed");
    assert!(payload.get("description").is_none());
}

#[tokio::test]
async fn repo_update_can_change_the_default_branch_and_the_issues_switch() {
    let world = with_token(vec![reply(
        200,
        repository(json!({"default_branch": "trunk"})),
    )]);
    ok(
        &world,
        "repo_update",
        json!({"repo": "o/r", "default_branch": "trunk", "has_issues": false}),
    )
    .await;
    assert_eq!(
        world.calls()[0].body,
        json!({"default_branch": "trunk", "has_issues": false})
    );
}

#[tokio::test]
async fn repo_update_refuses_an_empty_change_and_bad_values_before_any_request() {
    let world = without_token(vec![reply(200, repository(json!({})))]);
    let cases = [
        (
            json!({"repo": "o/r"}),
            "^Nothing to change: pass at least one of name,",
        ),
        (
            json!({"repo": "o/r", "name": "has space"}),
            "not a valid GitHub repository name",
        ),
        (
            json!({"repo": "o/r", "name": ".."}),
            "not a valid GitHub repository name",
        ),
        (
            json!({"repo": "o/r", "default_branch": "a..b"}),
            r"`default_branch` 'a\.\.b' is not a valid",
        ),
        (
            json!({"repo": "o/r", "private": "yes"}),
            "`private` must be true or false",
        ),
        (
            json!({"repo": "o/r", "archived": 1}),
            "`archived` must be true or false",
        ),
        (
            json!({"repo": "o/r", "description": 5}),
            "`description` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "repo_update", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_update_carries_githubs_reason_when_it_cannot_change_a_setting() {
    let world = with_token(vec![reply(
        422,
        json!({"message": "Validation Failed", "errors": [{"message": "Visibility change not allowed"}]}),
    )]);
    let m = refused(
        &world,
        "repo_update",
        json!({"repo": "o/r", "private": false}),
    )
    .await;
    assert_matches(
        &m,
        r"^GitHub could not update the repository o/r \(422\): Validation Failed \(Visibility change not allowed\)\.$",
    );
}

#[tokio::test]
async fn repo_delete_deletes_only_when_confirm_repeats_the_full_name_ignoring_case() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(
        &world,
        "repo_delete",
        json!({"repo": "o/r", "confirm": " O/R "}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "DELETE");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r");
    assert_eq!(
        payload,
        json!({"summary": "Deleted repository o/r.", "deleted": true, "full_name": "o/r"})
    );
}

#[tokio::test]
async fn repo_delete_refuses_without_a_matching_confirm_and_sends_nothing() {
    let world = with_token(vec![reply(204, none())]);
    let cases = [
        (
            json!({"repo": "o/r", "confirm": "o/other"}),
            r"^`confirm` must repeat the repository's full name, 'o/r', to delete it\. Nothing was deleted\.$",
        ),
        (
            json!({"repo": "o/r", "confirm": "r"}),
            "Nothing was deleted",
        ),
        (json!({"repo": "o/r"}), "`confirm` is required"),
        (
            json!({"repo": "o/r", "confirm": true}),
            "`confirm` must be a string",
        ),
        (json!({"confirm": "o/r"}), "`repo` is required"),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "repo_delete", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_delete_says_what_permission_deleting_needs_and_a_404() {
    let world = with_token(vec![
        reply(
            403,
            json!({"message": "Must have admin rights to Repository."}),
        ),
        reply(404, json!({"message": "Not Found"})),
    ]);
    let args = json!({"repo": "o/r", "confirm": "o/r"});
    assert_matches(
        &refused(&world, "repo_delete", args.clone()).await,
        r"^GitHub refused to delete o/r \(403\): Must have admin rights to Repository\.\. Deleting needs admin rights .*`delete_repo` scope",
    );
    assert_matches(
        &refused(&world, "repo_delete", args).await,
        r"could not find what it needs to delete the repository o/r \(404\)",
    );
}

#[tokio::test]
async fn repo_fork_forks_into_the_token_owners_account_by_default() {
    let world = with_token(vec![reply(
        202,
        repository(json!({"full_name": "me/r", "fork": true, "parent": {"full_name": "o/r"}})),
    )]);
    let payload = ok(&world, "repo_fork", json!({"repo": "o/r"})).await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/forks");
    assert_eq!(calls[0].body, json!({}));
    assert_matches(
        payload["summary"].as_str().unwrap(),
        r"^Forking o/r into me/r has started\. GitHub finishes the copy in the background",
    );
    assert_eq!(payload["parent"], "o/r");
    assert_eq!(payload["fork"], true);
}

#[tokio::test]
async fn repo_fork_passes_the_organization_the_new_name_and_the_branch_choice() {
    let world = with_token(vec![reply(
        202,
        repository(json!({"full_name": "acme/copy", "fork": true})),
    )]);
    ok(
        &world,
        "repo_fork",
        json!({"repo": "o/r", "organization": " acme ", "name": "copy", "default_branch_only": true}),
    )
    .await;
    assert_eq!(
        world.calls()[0].body,
        json!({"organization": "acme", "name": "copy", "default_branch_only": true})
    );
}

#[tokio::test]
async fn repo_fork_refuses_bad_arguments_before_any_request() {
    let world = with_token(vec![reply(202, repository(json!({})))]);
    let cases = [
        (json!({"repo": "nope"}), "is not a repository"),
        (
            json!({"repo": "o/r", "name": "a b"}),
            "`name` 'a b' is not a valid GitHub repository name",
        ),
        (
            json!({"repo": "o/r", "default_branch_only": "yes"}),
            "`default_branch_only` must be true or false",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "repo_fork", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_search_sends_the_query_and_the_paging_and_reports_each_match() {
    let world = with_token(vec![reply_with(
        200,
        json!({
            "total_count": 2,
            "incomplete_results": false,
            "items": [
                {"full_name": "a/mcp", "description": "First", "stargazers_count": 9, "language": "Go", "html_url": "https://github.com/a/mcp"},
                {"full_name": "b/mcp", "description": null, "stargazers_count": 1, "language": null, "html_url": "https://github.com/b/mcp"},
            ],
        }),
        &[(
            "link",
            r#"<https://api.github.com/search/repositories?page=2>; rel="next""#,
        )],
    )]);
    let payload = ok(
        &world,
        "repo_search",
        json!({"query": " mcp in:name ", "sort": "stars", "order": "asc", "per_page": 2}),
    )
    .await;
    let url = &world.calls()[0].url;
    assert_eq!(path_of(url), "https://api.github.com/search/repositories");
    let mut q = query(url);
    q.sort();
    let mut want: Vec<(String, String)> = [
        ("q", "mcp in:name"),
        ("sort", "stars"),
        ("order", "asc"),
        ("per_page", "2"),
        ("page", "1"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    want.sort();
    assert_eq!(q, want);
    assert_eq!(
        payload,
        json!({
            "summary": "2 repositories match 'mcp in:name'; showing 2.",
            "items": [
                {"full_name": "a/mcp", "description": "First", "stars": 9, "language": "Go", "url": "https://github.com/a/mcp"},
                {"full_name": "b/mcp", "stars": 1, "url": "https://github.com/b/mcp"},
            ],
            "total_count": 2,
            "incomplete": false,
            "page": 1,
            "per_page": 2,
            "has_more": true,
        })
    );
}

#[tokio::test]
async fn repo_search_leaves_order_out_unless_there_is_a_sort_and_reports_no_match_plainly() {
    let world = with_token(vec![reply(
        200,
        json!({"total_count": 0, "incomplete_results": true, "items": []}),
    )]);
    let payload = ok(
        &world,
        "repo_search",
        json!({"query": "zzzz", "order": "asc"}),
    )
    .await;
    let url = &world.calls()[0].url;
    assert!(!has_param(url, "order"));
    assert!(!has_param(url, "sort"));
    assert_eq!(payload["summary"], "No repositories match 'zzzz'.");
    assert_eq!(payload["incomplete"], true);
    assert_eq!(payload["has_more"], false);
}

#[tokio::test]
async fn repo_search_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, json!({"items": []}))]);
    let cases = [
        (json!({}), "`query` is required"),
        (json!({"query": "  "}), "`query` is required"),
        (
            json!({"query": "x", "sort": "best"}),
            "`sort` must be one of stars, forks, help-wanted-issues, updated, got 'best'",
        ),
        (
            json!({"query": "x", "order": "up"}),
            "`order` must be one of asc, desc",
        ),
        (
            json!({"query": "x", "per_page": 101}),
            "`per_page` must be between 1 and 100",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "repo_search", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn repo_search_passes_on_githubs_reason_and_its_rate_limit() {
    let world = with_token(vec![
        reply(
            422,
            json!({"message": "Validation Failed", "errors": [{"message": "The listed users cannot be searched"}]}),
        ),
        reply_with(
            403,
            json!({"message": "API rate limit exceeded"}),
            &[("retry-after", "20")],
        ),
    ]);
    assert_matches(
        &refused(&world, "repo_search", json!({"query": "user:ghost"})).await,
        r"search repositories for 'user:ghost' \(422\): Validation Failed \(The listed users cannot be searched\)",
    );
    assert_matches(
        &refused(&world, "repo_search", json!({"query": "mcp"})).await,
        r"rate limiting this token \(403\).*Wait 20 seconds",
    );
}
