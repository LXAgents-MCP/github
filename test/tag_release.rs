//! Ported from test/tag-release.test.ts.
use std::collections::BTreeMap;

use lxagents_github::models::address::Repo;
use lxagents_github::services::releases::find_release_by_tag;
use serde_json::{Value, json};

use crate::helpers::*;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TAGOBJ: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn repo() -> Repo {
    Repo {
        owner: "o".into(),
        name: "r".into(),
        full: "o/r".into(),
        path: "/repos/o/r".into(),
    }
}

/// A release as GitHub returns it.
fn release(overrides: Value) -> Value {
    let mut base = json!({
        "id": 42,
        "tag_name": "v1.0.0",
        "name": "First release",
        "draft": false,
        "prerelease": false,
        "html_url": "https://github.com/o/r/releases/tag/v1.0.0",
        "published_at": "2026-10-01T00:00:00Z",
        "body": "Notes",
        "assets": [{ "name": "app.zip", "size": 1024, "download_count": 7 }],
    });
    for (k, v) in obj(overrides) {
        base[k] = v;
    }
    base
}

/// The decoded query parameters of a URL.
fn query(url: &str) -> BTreeMap<String, String> {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            let d = |s: &str| {
                percent_encoding::percent_decode_str(s)
                    .decode_utf8_lossy()
                    .into_owned()
            };
            (d(k), d(v))
        })
        .collect()
}

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/* ------------------------------------------------------------------ tag_check */

#[tokio::test]
async fn tag_check_says_a_tag_exists_whether_annotated_and_what_it_points_at() {
    let world = with_token(vec![
        reply(
            200,
            json!({ "ref": "refs/tags/v1", "object": { "type": "commit", "sha": COMMIT } }),
        ),
        reply(
            200,
            json!({ "ref": "refs/tags/v2", "object": { "type": "tag", "sha": TAGOBJ } }),
        ),
    ]);
    let light = ok(&world, "tag_check", json!({ "repo": "o/r", "tag": "v1" })).await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/git/ref/tags/v1"
    );
    assert_eq!(
        light,
        json!({
            "summary": "Tag v1 exists in o/r (lightweight) and points at 0123456.",
            "exists": true,
            "tag": "v1",
            "sha": COMMIT,
            "annotated": false,
        })
    );
    let annotated = ok(&world, "tag_check", json!({ "repo": "o/r", "tag": "v2" })).await;
    assert_eq!(annotated["annotated"], true);
    assert_matches(
        annotated["summary"].as_str().unwrap(),
        r"\(annotated\) and points at fedcba9\.$",
    );
}

#[tokio::test]
async fn tag_check_treats_a_missing_tag_as_an_answer() {
    let world = with_token(vec![reply(404, json!({ "message": "Not Found" }))]);
    let done = call(&world, "tag_check", json!({ "repo": "o/r", "tag": "v9" }))
        .await
        .unwrap();
    assert!(done.ok);
    assert_eq!(
        done.payload,
        json!({ "summary": "Tag v9 does not exist in o/r.", "exists": false, "tag": "v9" })
    );
    expect_output(&def("tag_check"), &done.payload);
}

#[tokio::test]
async fn tag_check_still_refuses_a_real_failure_and_a_bad_argument() {
    let world = with_token(vec![reply(
        403,
        json!({ "message": "Resource not accessible" }),
    )]);
    assert_matches(
        &refused(&world, "tag_check", json!({ "repo": "o/r", "tag": "v1" })).await,
        r"GitHub refused to look for the tag 'v1' in o/r \(403\)",
    );
    assert_matches(
        &refused(&world, "tag_check", json!({ "repo": "o/r" })).await,
        r"`tag` is required",
    );
    assert_matches(
        &refused(&world, "tag_check", json!({ "repo": "o/r", "tag": "a..b" })).await,
        r"`tag` 'a\.\.b' is not a valid",
    );
    assert_eq!(world.calls().len(), 1);
}

/* ------------------------------------------------------------------ tag_get */

#[tokio::test]
async fn tag_get_reads_a_lightweight_tag_in_one_request() {
    let world = with_token(vec![reply(
        200,
        json!({ "object": { "type": "commit", "sha": COMMIT } }),
    )]);
    let payload = ok(&world, "tag_get", json!({ "repo": "o/r", "tag": "v1" })).await;
    assert_eq!(world.calls().len(), 1);
    assert_eq!(
        payload,
        json!({
            "summary": "Tag v1 is a lightweight tag pointing at 0123456.",
            "tag": "v1",
            "sha": COMMIT,
            "commit_sha": COMMIT,
            "target_type": "commit",
            "annotated": false,
        })
    );
}

#[tokio::test]
async fn tag_get_follows_an_annotated_tag_to_its_commit_message_and_tagger() {
    let world = with_token(vec![
        reply(200, json!({ "object": { "type": "tag", "sha": TAGOBJ } })),
        reply(
            200,
            json!({ "message": "Release 2\n", "tagger": { "name": "Ada", "date": "2026-10-01T00:00:00Z" }, "object": { "type": "commit", "sha": COMMIT } }),
        ),
    ]);
    let payload = ok(
        &world,
        "tag_get",
        json!({ "repo": "o/r", "tag": "release/2.0" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/git/ref/tags/release/2.0"
    );
    assert_eq!(
        calls[1].url,
        format!("https://api.github.com/repos/o/r/git/tags/{TAGOBJ}")
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Tag release/2.0 is annotated and points at 0123456.",
            "tag": "release/2.0",
            "sha": TAGOBJ,
            "commit_sha": COMMIT,
            "target_type": "commit",
            "annotated": true,
            "message": "Release 2\n",
            "tagger": "Ada",
            "date": "2026-10-01T00:00:00Z",
        })
    );
}

#[tokio::test]
async fn tag_get_says_there_is_no_such_tag_and_refuses_a_body_it_does_not_know() {
    let world = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(200, json!({ "unexpected": true })),
        reply(200, json!({ "object": { "type": "tag", "sha": TAGOBJ } })),
        reply(200, json!({ "nope": 1 })),
    ]);
    assert_matches(
        &refused(&world, "tag_get", json!({ "repo": "o/r", "tag": "v9" })).await,
        r"^There is no tag 'v9' in o/r\. List them with tag_get_all\.$",
    );
    assert_matches(
        &refused(&world, "tag_get", json!({ "repo": "o/r", "tag": "v1" })).await,
        r"the body was not a tag reference this server recognizes",
    );
    assert_matches(
        &refused(&world, "tag_get", json!({ "repo": "o/r", "tag": "v2" })).await,
        r"the body was not an annotated tag this server recognizes",
    );
}

/* ------------------------------------------------------------------ tag_get_all */

#[tokio::test]
async fn tag_get_all_lists_the_tags_with_their_commits_and_pages() {
    let world = with_token(vec![
        reply_with(
            200,
            json!([{ "name": "v2", "commit": { "sha": COMMIT } }, { "name": "v1", "commit": { "sha": TAGOBJ } }]),
            &[("link", r#"<x>; rel="next""#)],
        ),
        reply(200, json!([])),
    ]);
    let payload = ok(
        &world,
        "tag_get_all",
        json!({ "repo": "o/r", "per_page": 2 }),
    )
    .await;
    assert_eq!(
        query(&world.calls()[0].url),
        params(&[("page", "1"), ("per_page", "2")])
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 tags of o/r on page 1.",
            "items": [{ "name": "v2", "sha": COMMIT }, { "name": "v1", "sha": TAGOBJ }],
            "page": 1,
            "per_page": 2,
            "has_more": true,
        })
    );
    let empty = ok(&world, "tag_get_all", json!({ "repo": "o/r", "page": 3 })).await;
    assert_eq!(empty["summary"], "o/r has no tags on page 3.");
}

#[tokio::test]
async fn tag_get_all_refuses_bad_paging_before_any_request() {
    let world = without_token(vec![reply(200, json!([]))]);
    assert_matches(
        &refused(
            &world,
            "tag_get_all",
            json!({ "repo": "o/r", "per_page": 500 }),
        )
        .await,
        r"`per_page` must be between 1 and 100",
    );
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ tag_delete */

#[tokio::test]
async fn tag_delete_removes_the_tag_reference() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(&world, "tag_delete", json!({ "repo": "o/r", "tag": "v1" })).await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "DELETE");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/git/refs/tags/v1"
    );
    assert_eq!(
        payload,
        json!({ "summary": "Deleted tag v1 from o/r.", "deleted": true, "tag": "v1" })
    );
}

#[tokio::test]
async fn tag_delete_explains_a_422_and_refuses_a_bad_tag_before_any_request() {
    let world = with_token(vec![reply(
        422,
        json!({ "message": "Reference does not exist" }),
    )]);
    assert_matches(
        &refused(&world, "tag_delete", json!({ "repo": "o/r", "tag": "v9" })).await,
        r"^GitHub would not delete the tag 'v9' from o/r \(422\): Reference does not exist\. It probably does not exist",
    );
    assert_matches(
        &refused(&world, "tag_delete", json!({ "repo": "o/r", "tag": "x y" })).await,
        r"is not a valid",
    );
    assert_eq!(world.calls().len(), 1);
}

/* ------------------------------------------------------------------ services::releases */

#[tokio::test]
async fn find_release_by_tag_returns_the_published_release_then_searches_for_a_draft() {
    let world = with_token(vec![
        reply(200, release(json!({}))),
        reply(404, json!({ "message": "Not Found" })),
        reply(
            200,
            json!([
                release(json!({ "id": 1, "tag_name": "other" })),
                release(json!({ "id": 2, "tag_name": "v2", "draft": true }))
            ]),
        ),
        reply(404, json!({ "message": "Not Found" })),
        reply(200, json!([])),
    ]);
    let repo = repo();
    let found = find_release_by_tag(&world.ctx, &repo, "v1.0.0")
        .await
        .unwrap();
    assert_eq!(found["id"], 42);
    assert_eq!(world.calls().len(), 1);

    let draft = find_release_by_tag(&world.ctx, &repo, "v2").await.unwrap();
    assert_eq!(draft["id"], 2);
    assert_eq!(draft["draft"], true);
    let calls = world.calls();
    assert_eq!(
        calls[1].url,
        "https://api.github.com/repos/o/r/releases/tags/v2"
    );
    assert_eq!(
        query(&calls[2].url),
        params(&[("per_page", "100"), ("page", "1")])
    );

    let error = find_release_by_tag(&world.ctx, &repo, "ghost")
        .await
        .unwrap_err();
    assert_matches(
        error.message(),
        r"^There is no release for the tag 'ghost' in o/r\. Check the tag name, or list them with release_list\.$",
    );
}

#[tokio::test]
async fn find_release_by_tag_searches_up_to_three_pages() {
    let full = |n: i64| -> Value {
        Value::Array(
            (0..100)
                .map(|i| release(json!({ "id": n * 100 + i, "tag_name": format!("t{n}-{i}") })))
                .collect(),
        )
    };
    let world = with_token(vec![
        reply(404, json!({})),
        reply(200, full(1)),
        reply(200, full(2)),
        reply(
            200,
            json!([release(json!({ "id": 9, "tag_name": "wanted" }))]),
        ),
    ]);
    let found = find_release_by_tag(&world.ctx, &repo(), "wanted")
        .await
        .unwrap();
    assert_eq!(found["id"], 9);
    assert_eq!(world.calls().len(), 4);
}

/* ------------------------------------------------------------------ release_create */

#[tokio::test]
async fn release_create_sends_what_was_given_creating_the_tag_from_the_target() {
    let world = with_token(vec![reply(201, release(json!({ "draft": true })))]);
    let payload = ok(
        &world,
        "release_create",
        json!({
            "repo": "o/r",
            "tag": "v1.0.0",
            "target": "main",
            "name": " First release ",
            "body": "Notes\n",
            "draft": true,
            "prerelease": false,
            "generate_notes": true,
        }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/releases");
    assert_eq!(
        calls[0].body,
        json!({
            "tag_name": "v1.0.0",
            "target_commitish": "main",
            "name": "First release",
            "body": "Notes\n",
            "draft": true,
            "prerelease": false,
            "generate_release_notes": true,
        })
    );
    assert_eq!(
        payload["summary"],
        "Created draft release First release (tag v1.0.0) in o/r."
    );
    assert_eq!(payload["id"], 42);
    assert_eq!(
        payload["assets"],
        json!([{ "name": "app.zip", "size": 1024, "downloads": 7 }])
    );
}

#[tokio::test]
async fn release_create_sends_only_the_tag_by_default() {
    let mut body = release(json!({ "name": null, "body": null }));
    body.as_object_mut().unwrap().remove("assets");
    let world = with_token(vec![reply(201, body)]);
    let payload = ok(
        &world,
        "release_create",
        json!({ "repo": "o/r", "tag": "v1.0.0" }),
    )
    .await;
    assert_eq!(world.calls()[0].body, json!({ "tag_name": "v1.0.0" }));
    assert_eq!(
        payload["summary"],
        "Created release v1.0.0 (tag v1.0.0) in o/r."
    );
    assert_eq!(payload["assets"], json!([]));
    assert!(payload.get("name").is_none());
    assert!(payload.get("body").is_none());
}

#[tokio::test]
async fn release_create_says_when_a_release_for_the_tag_already_exists() {
    let world = with_token(vec![reply(
        422,
        json!({ "message": "Validation Failed", "errors": [{ "resource": "Release", "code": "already_exists", "field": "tag_name" }] }),
    )]);
    assert_matches(
        &refused(
            &world,
            "release_create",
            json!({ "repo": "o/r", "tag": "v1.0.0" }),
        )
        .await,
        r"A release for that tag already exists; read it with release_read, or use another tag\.$",
    );
}

#[tokio::test]
async fn release_create_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(201, release(json!({})))]);
    let cases = [
        (json!({ "repo": "o/r" }), r"`tag` is required"),
        (
            json!({ "repo": "o/r", "tag": "a b" }),
            r"`tag` 'a b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "tag": "v1", "target": "x y" }),
            r"`target` 'x y' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "tag": "v1", "draft": "yes" }),
            r"`draft` must be true or false",
        ),
        (
            json!({ "repo": "o/r", "tag": "v1", "body": 5 }),
            r"`body` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "release_create", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ release_read */

#[tokio::test]
async fn release_read_reads_a_release_by_tag() {
    let world = with_token(vec![reply(200, release(json!({})))]);
    let payload = ok(
        &world,
        "release_read",
        json!({ "repo": "o/r", "tag": "v1.0.0" }),
    )
    .await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/releases/tags/v1.0.0"
    );
    assert_eq!(
        payload["summary"],
        "o/r has release First release (tag v1.0.0)."
    );
    assert_eq!(payload["body"], "Notes");
    assert_eq!(payload["published_at"], "2026-10-01T00:00:00Z");
}

#[tokio::test]
async fn release_read_reads_the_latest_release_and_says_when_there_is_none() {
    let world = with_token(vec![
        reply(200, release(json!({ "prerelease": false }))),
        reply(404, json!({ "message": "Not Found" })),
    ]);
    ok(&world, "release_read", json!({ "repo": "o/r" })).await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/releases/latest"
    );
    assert_matches(
        &refused(&world, "release_read", json!({ "repo": "o/r" })).await,
        r"^o/r has no published release yet\. List what exists with release_list\.$",
    );
}

#[tokio::test]
async fn release_read_finds_a_draft_by_tag_through_the_list() {
    let world = with_token(vec![
        reply(404, json!({})),
        reply(
            200,
            json!([release(
                json!({ "id": 7, "tag_name": "v2", "draft": true, "name": "Next" })
            )]),
        ),
    ]);
    let payload = ok(
        &world,
        "release_read",
        json!({ "repo": "o/r", "tag": "v2" }),
    )
    .await;
    assert_eq!(payload["draft"], true);
    assert_eq!(payload["summary"], "o/r has draft release Next (tag v2).");
}

#[tokio::test]
async fn release_read_refuses_a_bad_tag_and_a_body_that_is_not_a_release() {
    let world = with_token(vec![reply(200, json!({ "nope": 1 }))]);
    assert_matches(
        &refused(
            &world,
            "release_read",
            json!({ "repo": "o/r", "tag": "../x" }),
        )
        .await,
        r"is not a valid",
    );
    assert_eq!(world.calls().len(), 0);
    assert_matches(
        &refused(
            &world,
            "release_read",
            json!({ "repo": "o/r", "tag": "v1" }),
        )
        .await,
        r"the body was not a release this server recognizes",
    );
}

/* ------------------------------------------------------------------ release_list */

#[tokio::test]
async fn release_list_lists_releases_with_their_state_and_pages() {
    let world = with_token(vec![reply_with(
        200,
        json!([
            release(json!({})),
            release(
                json!({ "id": 41, "tag_name": "v0.9.0", "draft": true, "name": null, "published_at": null })
            )
        ]),
        &[("link", r#"<x>; rel="next""#)],
    )]);
    let payload = ok(
        &world,
        "release_list",
        json!({ "repo": "o/r", "per_page": 2 }),
    )
    .await;
    assert_eq!(
        query(&world.calls()[0].url),
        params(&[("page", "1"), ("per_page", "2")])
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 releases of o/r on page 1.",
            "items": [
                { "id": 42, "tag": "v1.0.0", "name": "First release", "draft": false, "prerelease": false, "published_at": "2026-10-01T00:00:00Z" },
                { "id": 41, "tag": "v0.9.0", "draft": true, "prerelease": false },
            ],
            "page": 1,
            "per_page": 2,
            "has_more": true,
        })
    );
}

#[tokio::test]
async fn release_list_reports_an_empty_page() {
    let world = with_token(vec![reply(200, json!([]))]);
    let payload = ok(&world, "release_list", json!({ "repo": "o/r" })).await;
    assert_eq!(payload["summary"], "o/r has no releases on page 1.");
}

/* ------------------------------------------------------------------ release_delete */

#[tokio::test]
async fn release_delete_resolves_a_tag_to_its_id_and_deletes_leaving_the_tag() {
    let world = with_token(vec![reply(200, release(json!({}))), reply(204, none())]);
    let payload = ok(
        &world,
        "release_delete",
        json!({ "repo": "o/r", "tag": "v1.0.0" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[1].method, "DELETE");
    assert_eq!(calls[1].url, "https://api.github.com/repos/o/r/releases/42");
    assert_eq!(
        payload,
        json!({ "summary": "Deleted the release for v1.0.0 of o/r. Its tag was kept.", "deleted": true, "id": 42, "tag": "v1.0.0" })
    );
}

#[tokio::test]
async fn release_delete_takes_an_id_directly_and_makes_a_single_request() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(
        &world,
        "release_delete",
        json!({ "repo": "o/r", "release_id": 42 }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/releases/42");
    assert_eq!(
        payload["summary"],
        "Deleted the release 42 of o/r. Its tag was kept."
    );
    assert!(payload.get("tag").is_none());
}

#[tokio::test]
async fn release_delete_needs_exactly_one_of_tag_and_release_id_and_a_real_id() {
    let world = with_token(vec![reply(204, none())]);
    let cases = [
        (
            json!({ "repo": "o/r" }),
            r"^Give exactly one of `tag` and `release_id`\.$",
        ),
        (
            json!({ "repo": "o/r", "tag": "v1", "release_id": 4 }),
            r"Give exactly one of",
        ),
        (
            json!({ "repo": "o/r", "release_id": 0 }),
            r"`release_id` must be between 1 and",
        ),
        (
            json!({ "repo": "o/r", "release_id": "4" }),
            r"`release_id` must be a whole number",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&world, "release_delete", args).await, pattern);
    }
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn release_delete_says_when_there_is_no_release_for_the_tag() {
    let world = with_token(vec![reply(404, json!({})), reply(200, json!([]))]);
    assert_matches(
        &refused(
            &world,
            "release_delete",
            json!({ "repo": "o/r", "tag": "ghost" }),
        )
        .await,
        r"^There is no release for the tag 'ghost' in o/r",
    );
}
