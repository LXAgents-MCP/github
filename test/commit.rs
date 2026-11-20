//! Ported from test/commit.test.ts.
use std::collections::BTreeMap;

use lxagents_github::config::config::{MAX_COMMITS_SHOWN, MAX_DIFF_CHARS, MAX_FILES_SHOWN};
use serde_json::{Value, json};

use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const PARENT: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn merged(mut base: Value, overrides: Value) -> Value {
    for (k, v) in obj(overrides) {
        base[k] = v;
    }
    base
}

fn commit_body(overrides: Value) -> Value {
    merged(
        json!({
            "sha": SHA,
            "html_url": format!("https://github.com/o/r/commit/{SHA}"),
            "commit": {
                "message": "Add login\n\nThe long explanation\nover two lines.",
                "author": { "name": "Ada", "date": "2026-10-01T12:00:00Z" },
            },
            "parents": [{ "sha": PARENT }],
            "stats": { "additions": 12, "deletions": 3, "total": 15 },
            "files": [
                { "filename": "src/a.js", "status": "added", "additions": 10, "deletions": 0, "patch": "@@" },
                { "filename": "src/b.js", "status": "modified", "additions": 2, "deletions": 3 },
            ],
        }),
        overrides,
    )
}

fn many_files(n: usize) -> Value {
    Value::Array(
        (0..n)
            .map(|i| json!({ "filename": format!("f{i}.js"), "status": "modified", "additions": 1, "deletions": 0 }))
            .collect(),
    )
}

/// The path and the decoded query parameters of a URL.
fn split_url(url: &str) -> (String, BTreeMap<String, String>) {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let path = path
        .trim_start_matches("https://api.github.com")
        .to_string();
    let decode = |s: &str| {
        percent_encoding::percent_decode_str(&s.replace('+', " "))
            .decode_utf8_lossy()
            .into_owned()
    };
    let params = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect();
    (path, params)
}

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/* ------------------------------------------------------------------ commit_read */

#[tokio::test]
async fn commit_read_reports_the_message_author_parents_and_changed_files() {
    let world = with_token(vec![reply(200, commit_body(json!({})))]);
    let payload = ok(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "ref": "feat/login" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/commits/feat/login"
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Commit 0123456 by Ada: Add login",
            "sha": SHA,
            "message": "Add login\n\nThe long explanation\nover two lines.",
            "author": "Ada",
            "date": "2026-10-01T12:00:00Z",
            "parents": [PARENT],
            "additions": 12,
            "deletions": 3,
            "file_count": 2,
            "files": [
                { "filename": "src/a.js", "status": "added", "additions": 10, "deletions": 0 },
                { "filename": "src/b.js", "status": "modified", "additions": 2, "deletions": 3 },
            ],
            "truncated": false,
            "html_url": format!("https://github.com/o/r/commit/{SHA}"),
        })
    );
}

#[tokio::test]
async fn commit_read_defaults_to_head_and_copes_with_a_bare_commit() {
    let world = with_token(vec![reply(
        200,
        json!({ "sha": SHA, "commit": { "message": "" }, "parents": [] }),
    )]);
    let payload = ok(&world, "commit_read", json!({ "repo": "o/r" })).await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/commits/HEAD"
    );
    assert_eq!(payload["summary"], "Commit 0123456: (no message)");
    assert_eq!(payload["files"], json!([]));
    assert_eq!(payload["additions"], 0);
    assert_eq!(payload["file_count"], 0);
}

#[tokio::test]
async fn commit_read_asks_for_the_diff_with_its_own_media_type_and_cuts_a_long_one() {
    let diff = format!("diff --git a/x b/x\n{}", "+line\n".repeat(MAX_DIFF_CHARS));
    let world = with_token(vec![
        reply(200, commit_body(json!({}))),
        reply(200, Value::String(diff)),
    ]);
    let payload = ok(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "ref": SHA, "diff": true }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].header("Accept"),
        Some("application/vnd.github+json")
    );
    assert_eq!(
        calls[1].header("Accept"),
        Some("application/vnd.github.diff")
    );
    assert_eq!(calls[1].url, calls[0].url);
    assert_eq!(payload["truncated"], true);
    let text = payload["diff"].as_str().unwrap();
    assert!(text.starts_with("diff --git a/x b/x"));
    assert!(text.chars().count() <= MAX_DIFF_CHARS + 20);
}

#[tokio::test]
async fn commit_read_shows_the_first_hundred_files_and_reports_how_many() {
    let world = with_token(vec![reply(
        200,
        commit_body(json!({ "files": many_files(MAX_FILES_SHOWN + 20) })),
    )]);
    let payload = ok(&world, "commit_read", json!({ "repo": "o/r" })).await;
    assert_eq!(payload["files"].as_array().unwrap().len(), MAX_FILES_SHOWN);
    assert_eq!(payload["file_count"], MAX_FILES_SHOWN + 20);
    assert_eq!(payload["truncated"], true);
}

#[tokio::test]
async fn commit_read_says_a_huge_diff_timed_out_and_refuses_bad_arguments() {
    let world = with_token(vec![
        reply(200, commit_body(json!({}))),
        reply(504, json!({ "message": "Gateway Timeout" })),
    ]);
    let message = refused(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "diff": true }),
    )
    .await;
    assert_matches(
        &message,
        r"could not produce the diff of 'HEAD' in o/r \(504\)\. Very large diffs time out",
    );
    assert_eq!(world.calls().len(), 2);

    let message = refused(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "ref": "a b" }),
    )
    .await;
    assert_matches(&message, r"`ref` 'a b' is not a valid");
    let message = refused(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "diff": "yes" }),
    )
    .await;
    assert_matches(&message, r"`diff` must be true or false");
    assert_eq!(world.calls().len(), 2);
}

#[tokio::test]
async fn commit_read_reports_a_missing_commit_and_a_body_that_is_not_a_commit() {
    let world = with_token(vec![
        reply(
            404,
            json!({ "message": "No commit found for SHA: deadbeef" }),
        ),
        reply(200, json!({ "nope": 1 })),
    ]);
    let message = refused(
        &world,
        "commit_read",
        json!({ "repo": "o/r", "ref": "deadbeef" }),
    )
    .await;
    assert_matches(
        &message,
        r"could not find what it needs to read the commit 'deadbeef' of o/r \(404\): No commit found",
    );
    let message = refused(&world, "commit_read", json!({ "repo": "o/r" })).await;
    assert_matches(
        &message,
        r"the body was not a commit this server recognizes",
    );
}

/* ------------------------------------------------------------------ commit_list */

#[tokio::test]
async fn commit_list_sends_the_filters_and_the_paging_and_lists_each_commit() {
    let world = with_token(vec![reply_with(
        200,
        json!([
            { "sha": SHA, "commit": { "message": "Newest\n\nbody", "author": { "name": "Ada", "date": "2026-10-02T00:00:00Z" } } },
            { "sha": PARENT, "commit": { "message": "Older", "author": { "name": "Bob", "date": "2026-10-01T00:00:00Z" } } },
        ]),
        &[(
            "link",
            r#"<https://api.github.com/repos/o/r/commits?page=2>; rel="next""#,
        )],
    )]);
    let payload = ok(
        &world,
        "commit_list",
        json!({
            "repo": "o/r",
            "ref": "dev",
            "path": "/src/",
            "author": " ada ",
            "since": "2026-10-01",
            "until": "2026-10-31T23:59:59Z",
            "per_page": 2,
        }),
    )
    .await;
    let (path, query) = split_url(&world.calls()[0].url);
    assert_eq!(path, "/repos/o/r/commits");
    assert_eq!(
        query,
        params(&[
            ("sha", "dev"),
            ("path", "src"),
            ("author", "ada"),
            ("since", "2026-10-01"),
            ("until", "2026-10-31T23:59:59Z"),
            ("page", "1"),
            ("per_page", "2"),
        ])
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 commits on dev of o/r on page 1.",
            "items": [
                { "sha": SHA, "message": "Newest", "author": "Ada", "date": "2026-10-02T00:00:00Z" },
                { "sha": PARENT, "message": "Older", "author": "Bob", "date": "2026-10-01T00:00:00Z" },
            ],
            "page": 1,
            "per_page": 2,
            "has_more": true,
        })
    );
}

#[tokio::test]
async fn commit_list_sends_no_filter_by_default_and_reports_an_empty_page() {
    let world = with_token(vec![reply(200, json!([]))]);
    let payload = ok(
        &world,
        "commit_list",
        json!({ "repo": "o/r", "ref": "dev", "page": 9 }),
    )
    .await;
    let (_, query) = split_url(&world.calls()[0].url);
    assert_eq!(
        query,
        params(&[("sha", "dev"), ("page", "9"), ("per_page", "30")])
    );
    assert_eq!(
        payload["summary"],
        "No commits on dev of o/r match on page 9."
    );
    assert_eq!(payload["items"], json!([]));
}

#[tokio::test]
async fn commit_list_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, json!([]))]);
    let cases = [
        (
            json!({ "repo": "o/r", "since": "yesterday" }),
            r"`since` must be an ISO 8601 date",
        ),
        (
            json!({ "repo": "o/r", "until": "soon" }),
            r"`until` must be an ISO 8601 date",
        ),
        (
            json!({ "repo": "o/r", "path": "../x" }),
            r"is not a path inside the repository",
        ),
        (
            json!({ "repo": "o/r", "ref": "a..b" }),
            r"`ref` 'a\.\.b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "author": 3 }),
            r"`author` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "commit_list", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ commit_compare */

fn comparison(overrides: Value) -> Value {
    merged(
        json!({
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "html_url": "https://github.com/o/r/compare/main...feat",
            "commits": [
                { "sha": PARENT, "commit": { "message": "First\n\nbody", "author": { "name": "Ada" } } },
                { "sha": SHA, "commit": { "message": "Second", "author": { "name": "Bob" } } },
            ],
            "files": [{ "filename": "a.js", "status": "added", "additions": 4, "deletions": 0 }],
        }),
        overrides,
    )
}

#[tokio::test]
async fn commit_compare_reports_how_far_ahead_the_head_is_the_commits_and_the_files() {
    let world = with_token(vec![reply(200, comparison(json!({})))]);
    let payload = ok(
        &world,
        "commit_compare",
        json!({ "repo": "o/r", "base": "main", "head": "feat/login" }),
    )
    .await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/compare/main...feat/login"
    );
    assert_eq!(
        payload,
        json!({
            "summary": "feat/login is 2 commits ahead of main.",
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                { "sha": PARENT, "message": "First", "author": "Ada" },
                { "sha": SHA, "message": "Second", "author": "Bob" },
            ],
            "file_count": 1,
            "files": [{ "filename": "a.js", "status": "added", "additions": 4, "deletions": 0 }],
            "truncated": false,
            "html_url": "https://github.com/o/r/compare/main...feat",
        })
    );
}

#[tokio::test]
async fn commit_compare_words_each_status() {
    let world = with_token(vec![
        reply(
            200,
            comparison(
                json!({ "status": "identical", "ahead_by": 0, "total_commits": 0, "commits": [], "files": [] }),
            ),
        ),
        reply(
            200,
            comparison(json!({ "status": "behind", "ahead_by": 0, "behind_by": 1 })),
        ),
        reply(
            200,
            comparison(json!({ "status": "diverged", "ahead_by": 3, "behind_by": 4 })),
        ),
        reply(200, comparison(json!({ "status": "ahead", "ahead_by": 1 }))),
        reply(200, comparison(json!({ "status": "mystery" }))),
    ]);
    let args = json!({ "repo": "o/r", "base": "main", "head": "x" });
    for expected in [
        "x and main are identical.",
        "x is 1 commit behind main.",
        "x has diverged from main: 3 ahead, 4 behind.",
        "x is 1 commit ahead of main.",
        "x compared with main: mystery.",
    ] {
        let done = call(&world, "commit_compare", args.clone()).await.unwrap();
        assert_eq!(done.payload["summary"], expected);
    }
}

#[tokio::test]
async fn commit_compare_keeps_the_newest_commits_and_the_first_files_when_there_are_many() {
    let commits: Vec<Value> = (0..MAX_COMMITS_SHOWN + 10)
        .map(|i| json!({ "sha": format!("sha{i}"), "commit": { "message": format!("c{i}") } }))
        .collect();
    let total = commits.len();
    let world = with_token(vec![reply(
        200,
        comparison(
            json!({ "commits": commits, "total_commits": total, "files": many_files(MAX_FILES_SHOWN + 5) }),
        ),
    )]);
    let payload = ok(
        &world,
        "commit_compare",
        json!({ "repo": "o/r", "base": "main", "head": "x" }),
    )
    .await;
    let shown = payload["commits"].as_array().unwrap();
    assert_eq!(shown.len(), MAX_COMMITS_SHOWN);
    assert_eq!(shown[0]["message"], "c10");
    assert_eq!(
        shown.last().unwrap()["message"],
        format!("c{}", MAX_COMMITS_SHOWN + 9)
    );
    assert_eq!(payload["total_commits"], MAX_COMMITS_SHOWN + 10);
    assert_eq!(payload["files"].as_array().unwrap().len(), MAX_FILES_SHOWN);
    assert_eq!(payload["file_count"], MAX_FILES_SHOWN + 5);
    assert_eq!(payload["truncated"], true);
}

#[tokio::test]
async fn commit_compare_reports_a_missing_ref_and_refuses_bad_arguments() {
    let world = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(200, json!("nope")),
    ]);
    let message = refused(
        &world,
        "commit_compare",
        json!({ "repo": "o/r", "base": "main", "head": "ghost" }),
    )
    .await;
    assert_matches(
        &message,
        r"could not find what it needs to compare 'ghost' with 'main' in o/r \(404\)",
    );
    let message = refused(
        &world,
        "commit_compare",
        json!({ "repo": "o/r", "base": "main", "head": "x" }),
    )
    .await;
    assert_matches(
        &message,
        r"the body was not a comparison this server recognizes",
    );
    assert_eq!(world.calls().len(), 2);

    let cases = [
        (json!({ "repo": "o/r", "head": "x" }), r"`base` is required"),
        (
            json!({ "repo": "o/r", "base": "main" }),
            r"`head` is required",
        ),
        (
            json!({ "repo": "o/r", "base": "main", "head": "a...b" }),
            r"`head` 'a\.\.\.b' is not a valid",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "commit_compare", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 2);
}
