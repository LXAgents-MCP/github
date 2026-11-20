//! Ported from test/issue.test.ts.
use serde_json::{Value, json};

use crate::helpers::*;

/// An issue as GitHub returns it.
fn issue(overrides: Value) -> Value {
    let mut base = json!({
        "number": 42,
        "title": "Login fails",
        "state": "open",
        "html_url": "https://github.com/o/r/issues/42",
        "user": { "login": "ada" },
        "labels": [{ "name": "bug" }],
        "assignees": [{ "login": "bob" }],
        "milestone": { "title": "v1" },
        "comments": 2,
        "created_at": "2026-10-01T00:00:00Z",
        "updated_at": "2026-10-02T00:00:00Z",
        "body": "Steps\nto reproduce",
    });
    for (k, v) in obj(overrides) {
        base[k] = v;
    }
    base
}

async fn refuses(world: &World, tool: &str, cases: Vec<(Value, &str)>) {
    for (args, pattern) in cases {
        assert_matches(&refused(world, tool, args).await, pattern);
    }
}

/* ------------------------------------------------------------------ issue_open */

#[tokio::test]
async fn issue_open_opens_an_issue_with_everything_it_was_given() {
    let world = with_token(vec![reply(
        201,
        issue(
            json!({ "labels": [{ "name": "bug" }, { "name": "ui" }], "assignees": [{ "login": "bob" }] }),
        ),
    )]);
    let payload = ok(
        &world,
        "issue_open",
        json!({
            "repo": "o/r",
            "title": " Login fails ",
            "body": "Steps\n",
            "labels": ["bug", "ui", "bug"],
            "assignees": ["bob"],
            "milestone": 3,
        }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/issues");
    assert_eq!(
        calls[0].body,
        json!({ "title": "Login fails", "body": "Steps\n", "labels": ["bug", "ui"], "assignees": ["bob"], "milestone": 3 })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Opened issue #42 in o/r: Login fails.",
            "number": 42,
            "title": "Login fails",
            "state": "open",
            "html_url": "https://github.com/o/r/issues/42",
            "labels": ["bug", "ui"],
            "assignees": ["bob"],
        })
    );
}

#[tokio::test]
async fn issue_open_sends_only_the_title_by_default_and_says_what_github_dropped() {
    let world = with_token(vec![
        reply(201, issue(json!({ "labels": [], "assignees": [] }))),
        reply(
            201,
            issue(json!({ "labels": [{ "name": "bug" }], "assignees": [] })),
        ),
    ]);
    let plain = ok(&world, "issue_open", json!({ "repo": "o/r", "title": "T" })).await;
    assert_eq!(world.calls()[0].body, json!({ "title": "T" }));
    assert_eq!(plain["summary"], "Opened issue #42 in o/r: Login fails.");

    let partial = ok(
        &world,
        "issue_open",
        json!({ "repo": "o/r", "title": "T", "labels": ["bug", "nope"], "assignees": ["ghost"] }),
    )
    .await;
    assert_matches(
        partial["summary"].as_str().unwrap(),
        r"GitHub did not apply: label nope, assignee ghost\. The token may lack push access",
    );
}

#[tokio::test]
async fn issue_open_points_at_repo_update_when_issues_are_disabled_and_refuses_bad_arguments() {
    let world = with_token(vec![reply(
        410,
        json!({ "message": "Issues are disabled for this repo" }),
    )]);
    assert_matches(
        &refused(&world, "issue_open", json!({ "repo": "o/r", "title": "T" })).await,
        r"^Issues are disabled in o/r \(410\): Issues are disabled for this repo\. Turn them on with repo_update \(has_issues\)\.$",
    );
    assert_eq!(world.calls().len(), 1);
    refuses(
        &world,
        "issue_open",
        vec![
            (json!({ "repo": "o/r" }), r"`title` is required"),
            (
                json!({ "repo": "o/r", "title": "T", "labels": "bug" }),
                r"`labels` must be a list of strings",
            ),
            (
                json!({ "repo": "o/r", "title": "T", "milestone": 0 }),
                r"`milestone` must be between 1 and",
            ),
            (
                json!({ "repo": "o/r", "title": "T", "body": 4 }),
                r"`body` must be a string",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 1);
}

/* ------------------------------------------------------------------ issue_read */

#[tokio::test]
async fn issue_read_reports_the_issue_its_labels_assignees_and_body() {
    let world = with_token(vec![reply(
        200,
        issue(
            json!({ "closed_at": "2026-10-03T00:00:00Z", "state": "closed", "state_reason": "completed" }),
        ),
    )]);
    let payload = ok(&world, "issue_read", json!({ "repo": "o/r", "number": 42 })).await;
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/issues/42");
    assert_eq!(
        payload,
        json!({
            "summary": "Issue #42 in o/r is closed: Login fails.",
            "number": 42,
            "title": "Login fails",
            "state": "closed",
            "state_reason": "completed",
            "author": "ada",
            "labels": ["bug"],
            "assignees": ["bob"],
            "milestone": "v1",
            "comment_count": 2,
            "is_pull_request": false,
            "html_url": "https://github.com/o/r/issues/42",
            "created_at": "2026-10-01T00:00:00Z",
            "updated_at": "2026-10-02T00:00:00Z",
            "closed_at": "2026-10-03T00:00:00Z",
            "body": "Steps\nto reproduce",
        })
    );
}

#[tokio::test]
async fn issue_read_fetches_the_first_thirty_comments_only_when_asked_and_only_if_any() {
    let world = with_token(vec![
        reply(200, issue(json!({ "comments": 45 }))),
        reply(
            200,
            json!([
                { "id": 1, "user": { "login": "bob" }, "created_at": "2026-10-02T01:00:00Z", "body": "First\nline two" },
                { "id": 2, "user": null, "body": null },
            ]),
        ),
        reply(200, issue(json!({ "comments": 0 }))),
    ]);
    let args = json!({ "repo": "o/r", "number": 42, "comments": true });
    let with_comments = ok(&world, "issue_read", args.clone()).await;
    let calls = world.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[1].url,
        "https://api.github.com/repos/o/r/issues/42/comments?per_page=30"
    );
    assert_eq!(
        with_comments["comments"],
        json!([
            { "id": 1, "author": "bob", "created_at": "2026-10-02T01:00:00Z", "body": "First\nline two" },
            { "id": 2, "body": "" },
        ])
    );
    assert_matches(
        with_comments["summary"].as_str().unwrap(),
        r"; showing the first 2 comments\.$",
    );

    let none = ok(&world, "issue_read", args).await;
    assert_eq!(world.calls().len(), 3);
    assert!(none.get("comments").is_none());
}

#[tokio::test]
async fn issue_read_says_when_the_number_is_a_pull_request() {
    let world = with_token(vec![reply(
        200,
        issue(json!({ "pull_request": { "url": "x" } })),
    )]);
    let payload = ok(&world, "issue_read", json!({ "repo": "o/r", "number": 42 })).await;
    assert_eq!(payload["is_pull_request"], true);
    assert_matches(
        payload["summary"].as_str().unwrap(),
        r"^Pull request #42 in o/r is open",
    );
}

#[tokio::test]
async fn issue_read_reports_a_missing_issue_and_refuses_bad_arguments() {
    let world = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(200, json!({ "nope": 1 })),
    ]);
    let args = json!({ "repo": "o/r", "number": 9 });
    assert_matches(
        &refused(&world, "issue_read", args.clone()).await,
        r"could not find what it needs to read issue #9 of o/r \(404\)",
    );
    assert_matches(
        &refused(&world, "issue_read", args).await,
        r"the body was not an issue this server recognizes",
    );
    assert_eq!(world.calls().len(), 2);
    refuses(
        &world,
        "issue_read",
        vec![
            (json!({ "repo": "o/r" }), r"`number` is required"),
            (
                json!({ "repo": "o/r", "number": "42" }),
                r"`number` must be a whole number",
            ),
            (
                json!({ "repo": "o/r", "number": 0 }),
                r"`number` must be between 1 and",
            ),
            (
                json!({ "repo": "o/r", "number": 1, "comments": "yes" }),
                r"`comments` must be true or false",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 2);
}

/* ------------------------------------------------------------------ issue_comment */

#[tokio::test]
async fn issue_comment_posts_the_body_as_given() {
    let world = with_token(vec![reply(
        201,
        json!({ "id": 99, "html_url": "https://github.com/o/r/issues/42#issuecomment-99" }),
    )]);
    let payload = ok(
        &world,
        "issue_comment",
        json!({ "repo": "o/r", "number": 42, "body": "  Thanks!\n" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/comments"
    );
    assert_eq!(calls[0].body, json!({ "body": "  Thanks!\n" }));
    assert_eq!(
        payload,
        json!({ "summary": "Commented on #42 in o/r.", "id": 99, "html_url": "https://github.com/o/r/issues/42#issuecomment-99" })
    );
}

#[tokio::test]
async fn issue_comment_says_when_locked_and_refuses_a_blank_body() {
    let world = with_token(vec![reply(
        403,
        json!({ "message": "Unable to create comment because issue is locked." }),
    )]);
    assert_matches(
        &refused(
            &world,
            "issue_comment",
            json!({ "repo": "o/r", "number": 42, "body": "x" }),
        )
        .await,
        r"\(403\): Unable to create comment because issue is locked\.\. The conversation is locked\.$",
    );
    refuses(
        &world,
        "issue_comment",
        vec![
            (
                json!({ "repo": "o/r", "number": 42 }),
                r"`body` is required",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "body": "  " }),
                r"`body` is required and must not be blank",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 1);
}

/* ------------------------------------------------------------------ issue_close */

#[tokio::test]
async fn issue_close_closes_as_completed_by_default() {
    let world = with_token(vec![reply(
        200,
        issue(json!({ "state": "closed", "state_reason": "completed" })),
    )]);
    let payload = ok(
        &world,
        "issue_close",
        json!({ "repo": "o/r", "number": 42 }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PATCH");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/issues/42");
    assert_eq!(
        calls[0].body,
        json!({ "state": "closed", "state_reason": "completed" })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Closed issue #42 in o/r as completed.",
            "number": 42,
            "state": "closed",
            "state_reason": "completed",
            "html_url": "https://github.com/o/r/issues/42",
        })
    );
}

#[tokio::test]
async fn issue_close_can_close_as_not_planned_commenting_first() {
    let world = with_token(vec![
        reply(201, json!({ "id": 1, "html_url": "x" })),
        reply(
            200,
            issue(json!({ "state": "closed", "state_reason": "not_planned" })),
        ),
    ]);
    let payload = ok(
        &world,
        "issue_close",
        json!({ "repo": "o/r", "number": 42, "reason": "not_planned", "comment": "Out of scope." }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/comments"
    );
    assert_eq!(calls[0].body, json!({ "body": "Out of scope." }));
    assert_eq!(calls[1].body["state_reason"], "not_planned");
    assert_eq!(
        payload["summary"],
        "Closed issue #42 in o/r as not planned."
    );
}

#[tokio::test]
async fn issue_close_says_the_comment_was_already_posted_when_closing_then_fails() {
    let world = with_token(vec![
        reply(201, json!({ "id": 1, "html_url": "x" })),
        reply(403, json!({ "message": "Must have push access" })),
    ]);
    assert_matches(
        &refused(
            &world,
            "issue_close",
            json!({ "repo": "o/r", "number": 42, "comment": "Bye" }),
        )
        .await,
        r"close issue #42 of o/r \(403\).* The comment had already been posted\.$",
    );
}

#[tokio::test]
async fn issue_close_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, issue(json!({})))]);
    refuses(
        &world,
        "issue_close",
        vec![
            (
                json!({ "repo": "o/r", "number": 42, "reason": "wontfix" }),
                r"`reason` must be one of completed, not_planned, duplicate, got 'wontfix'",
            ),
            (json!({ "repo": "o/r" }), r"`number` is required"),
            (
                json!({ "repo": "o/r", "number": 1, "comment": 5 }),
                r"`comment` must be a string",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ issue_assign */

#[tokio::test]
async fn issue_assign_adds_assignees_by_default_and_reports_who_is_assigned() {
    let world = with_token(vec![reply(
        201,
        issue(json!({ "assignees": [{ "login": "bob" }, { "login": "cy" }] })),
    )]);
    let payload = ok(
        &world,
        "issue_assign",
        json!({ "repo": "o/r", "number": 42, "assignees": ["cy"] }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/assignees"
    );
    assert_eq!(calls[0].body, json!({ "assignees": ["cy"] }));
    assert_eq!(
        payload,
        json!({ "summary": "Assignees of #42 in o/r: bob, cy.", "number": 42, "assignees": ["bob", "cy"] })
    );
}

#[tokio::test]
async fn issue_assign_removes_with_a_body_on_delete_and_replaces_with_a_patch() {
    let world = with_token(vec![
        reply(200, issue(json!({ "assignees": [] }))),
        reply(200, issue(json!({ "assignees": [{ "login": "ann" }] }))),
    ]);
    let removed = ok(
        &world,
        "issue_assign",
        json!({ "repo": "o/r", "number": 42, "assignees": ["bob"], "action": "remove" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "DELETE");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/assignees"
    );
    assert_eq!(calls[0].body, json!({ "assignees": ["bob"] }));
    assert_eq!(removed["summary"], "Assignees of #42 in o/r: none.");

    let set = ok(
        &world,
        "issue_assign",
        json!({ "repo": "o/r", "number": 42, "assignees": ["ann"], "action": "set" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[1].method, "PATCH");
    assert_eq!(calls[1].url, "https://api.github.com/repos/o/r/issues/42");
    assert_eq!(set["assignees"], json!(["ann"]));
}

#[tokio::test]
async fn issue_assign_reports_the_people_github_did_not_assign_or_remove() {
    let world = with_token(vec![
        reply(201, issue(json!({ "assignees": [{ "login": "bob" }] }))),
        reply(200, issue(json!({ "assignees": [{ "login": "bob" }] }))),
    ]);
    let add = ok(
        &world,
        "issue_assign",
        json!({ "repo": "o/r", "number": 42, "assignees": ["bob", "ghost"] }),
    )
    .await;
    assert_eq!(add["not_applied"], json!(["ghost"]));
    assert_matches(
        add["summary"].as_str().unwrap(),
        r"GitHub did not assign: ghost\. They may not be collaborators",
    );

    // The summary ends at the sentence: 0.7.0 left a trailing space after it here.
    let remove = ok(
        &world,
        "issue_assign",
        json!({ "repo": "o/r", "number": 42, "assignees": ["bob"], "action": "remove" }),
    )
    .await;
    assert_eq!(remove["not_applied"], json!(["bob"]));
    assert_matches(
        remove["summary"].as_str().unwrap(),
        r"GitHub did not remove: bob\.$",
    );
}

#[tokio::test]
async fn issue_assign_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(201, issue(json!({})))]);
    let eleven: Vec<String> = (0..11).map(|i| format!("u{i}")).collect();
    refuses(
        &world,
        "issue_assign",
        vec![
            (
                json!({ "repo": "o/r", "number": 42 }),
                r"`assignees` is required",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "assignees": [] }),
                r"`assignees` is required",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "assignees": eleven }),
                r"at most 10 values, got 11",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "assignees": ["a"], "action": "toggle" }),
                r"`action` must be one of add, remove, set",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ issue_label */

#[tokio::test]
async fn issue_label_adds_labels_and_reports_every_label_now_on_the_issue() {
    let world = with_token(vec![reply(
        200,
        json!([{ "name": "bug" }, { "name": "good first issue" }]),
    )]);
    let payload = ok(
        &world,
        "issue_label",
        json!({ "repo": "o/r", "number": 42, "labels": ["good first issue"] }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/labels"
    );
    assert_eq!(calls[0].body, json!({ "labels": ["good first issue"] }));
    assert_eq!(
        payload,
        json!({ "summary": "Labels on #42 in o/r: bug, good first issue.", "number": 42, "labels": ["bug", "good first issue"] })
    );
}

#[tokio::test]
async fn issue_label_replaces_with_put_and_reports_a_label_github_did_not_apply() {
    let world = with_token(vec![reply(200, json!([{ "name": "bug" }]))]);
    let payload = ok(
        &world,
        "issue_label",
        json!({ "repo": "o/r", "number": 42, "labels": ["bug", "nope"], "action": "set" }),
    )
    .await;
    assert_eq!(world.calls()[0].method, "PUT");
    assert_eq!(payload["not_applied"], json!(["nope"]));
    assert_matches(
        payload["summary"].as_str().unwrap(),
        r"GitHub did not apply: nope\.$",
    );
}

#[tokio::test]
async fn issue_label_removes_one_label_at_a_time_encoding_the_name_and_treats_404_as_off() {
    let world = with_token(vec![
        reply(200, json!([{ "name": "ui" }])),
        reply(404, json!({ "message": "Label does not exist" })),
    ]);
    let payload = ok(
        &world,
        "issue_label",
        json!({ "repo": "o/r", "number": 42, "labels": ["good first issue", "gone"], "action": "remove" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "DELETE");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/labels/good%20first%20issue"
    );
    assert_eq!(
        calls[1].url,
        "https://api.github.com/repos/o/r/issues/42/labels/gone"
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Labels on #42 in o/r: ui. Not on the issue to begin with: gone.",
            "number": 42,
            "labels": ["ui"],
            "not_applied": ["gone"],
        })
    );
}

#[tokio::test]
async fn issue_label_reads_the_labels_when_every_removal_was_a_404() {
    let world = with_token(vec![
        reply(404, json!({})),
        reply(200, json!([{ "name": "bug" }])),
    ]);
    let payload = ok(
        &world,
        "issue_label",
        json!({ "repo": "o/r", "number": 42, "labels": ["x"], "action": "remove" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[1].method, "GET");
    assert_eq!(
        calls[1].url,
        "https://api.github.com/repos/o/r/issues/42/labels"
    );
    assert_eq!(payload["labels"], json!(["bug"]));
}

#[tokio::test]
async fn issue_label_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, json!([]))]);
    refuses(
        &world,
        "issue_label",
        vec![
            (
                json!({ "repo": "o/r", "number": 42 }),
                r"`labels` is required",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "labels": ["a"], "action": "x" }),
                r"`action` must be one of add, remove, set",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

/* ------------------------------------------------------------------ issue_lock */

#[tokio::test]
async fn issue_lock_locks_with_a_reason() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(
        &world,
        "issue_lock",
        json!({ "repo": "o/r", "number": 42, "reason": "too heated" }),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/42/lock"
    );
    assert_eq!(calls[0].body, json!({ "lock_reason": "too heated" }));
    assert_eq!(
        payload,
        json!({ "summary": "Locked #42 in o/r as too heated.", "number": 42, "locked": true, "reason": "too heated" })
    );
}

#[tokio::test]
async fn issue_lock_locks_with_no_body_when_there_is_no_reason_and_unlocks() {
    let world = with_token(vec![reply(204, none()), reply(204, none())]);
    let locked = ok(&world, "issue_lock", json!({ "repo": "o/r", "number": 42 })).await;
    assert!(world.calls()[0].raw_body.is_none());
    assert_eq!(locked["summary"], "Locked #42 in o/r.");
    assert!(locked.get("reason").is_none());

    let unlocked = ok(
        &world,
        "issue_lock",
        json!({ "repo": "o/r", "number": 42, "locked": false, "reason": "spam" }),
    )
    .await;
    assert_eq!(world.calls()[1].method, "DELETE");
    assert_eq!(
        unlocked,
        json!({ "summary": "Unlocked #42 in o/r.", "number": 42, "locked": false })
    );
}

#[tokio::test]
async fn issue_lock_refuses_bad_arguments_and_reports_a_permission_it_lacks() {
    let world = with_token(vec![reply(
        403,
        json!({ "message": "Must have push access" }),
    )]);
    refuses(
        &world,
        "issue_lock",
        vec![
            (
                json!({ "repo": "o/r", "number": 42, "reason": "boring" }),
                r"`reason` must be one of off-topic, too heated, resolved, spam",
            ),
            (
                json!({ "repo": "o/r", "number": 42, "locked": "yes" }),
                r"`locked` must be true or false",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
    assert_matches(
        &refused(&world, "issue_lock", json!({ "repo": "o/r", "number": 42 })).await,
        r"GitHub refused to lock #42 of o/r \(403\): Must have push access",
    );
}
