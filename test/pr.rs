//! Ported from test/pr.test.ts.

use serde_json::{Value, json};

use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// Merge `overrides` over `base`.
fn merged(mut base: Value, overrides: Value) -> Value {
    if let (Some(map), Value::Object(extra)) = (base.as_object_mut(), overrides) {
        map.extend(extra);
    }
    base
}

/// A pull request as GitHub returns it.
fn pull(overrides: Value) -> Value {
    merged(
        json!({
            "number": 7,
            "title": "Add login",
            "state": "open",
            "draft": false,
            "merged": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "user": { "login": "ada" },
            "head": { "ref": "feat/login", "sha": SHA },
            "base": { "ref": "main" },
            "commits": 3,
            "changed_files": 2,
            "additions": 20,
            "deletions": 4,
            "labels": [{ "name": "feature" }],
            "requested_reviewers": [{ "login": "bob" }],
            "requested_teams": [{ "slug": "platform" }],
            "body": "Adds login",
            "html_url": "https://github.com/o/r/pull/7",
        }),
        overrides,
    )
}

/* ------------------------------------------------------------------ pr_open */

#[tokio::test]
async fn pr_open_asks_for_the_default_branch_then_opens() {
    let w = with_token(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(201, pull(json!({ "draft": true }))),
    ]);
    let payload = ok(
        &w,
        "pr_open",
        json!({ "repo": "o/r", "title": " Add login ", "head": "feat/login", "body": "Why\n", "draft": true }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r");
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].url, "https://api.github.com/repos/o/r/pulls");
    assert_eq!(
        calls[1].body,
        json!({ "title": "Add login", "head": "feat/login", "base": "main", "body": "Why\n", "draft": true })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Opened draft pull request #7 in o/r: feat/login into main.",
            "number": 7,
            "title": "Add login",
            "state": "open",
            "draft": true,
            "head": "feat/login",
            "base": "main",
            "html_url": "https://github.com/o/r/pull/7",
        })
    );
}

#[tokio::test]
async fn pr_open_with_a_base_makes_one_request_and_takes_a_fork_branch() {
    let w = with_token(vec![reply(201, pull(json!({})))]);
    let payload = ok(
        &w,
        "pr_open",
        json!({ "repo": "o/r", "title": "T", "head": "ada:feat/x", "base": "release/1.0" }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].body,
        json!({ "title": "T", "head": "ada:feat/x", "base": "release/1.0" })
    );
    assert_eq!(
        payload["summary"],
        "Opened pull request #7 in o/r: ada:feat/x into release/1.0."
    );
}

#[tokio::test]
async fn pr_open_explains_existing_pull_request_and_no_commits() {
    let w = with_token(vec![
        reply(
            422,
            json!({ "message": "Validation Failed", "errors": [{ "message": "A pull request already exists for o:feat." }] }),
        ),
        reply(
            422,
            json!({ "message": "Validation Failed", "errors": [{ "message": "No commits between main and feat" }] }),
        ),
    ]);
    let args = json!({ "repo": "o/r", "title": "T", "head": "feat", "base": "main" });
    assert_matches(
        &refused(&w, "pr_open", args.clone()).await,
        r"\(422\).*A pull request already exists.*One is already open; find it with pr_read\.$",
    );
    assert_matches(
        &refused(&w, "pr_open", args).await,
        r"No commits between main and feat.*The branches have no difference to propose\.$",
    );
}

#[tokio::test]
async fn pr_open_refuses_bad_arguments_and_a_missing_default_branch() {
    let w = with_token(vec![reply(200, json!({}))]);
    let cases = [
        (
            json!({ "repo": "o/r", "head": "x" }),
            r"`title` is required",
        ),
        (
            json!({ "repo": "o/r", "title": "T" }),
            r"`head` is required",
        ),
        (
            json!({ "repo": "o/r", "title": "T", "head": "a b" }),
            r"`head` 'a b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "title": "T", "head": "-bad:branch" }),
            r"`head` '-bad:branch' is not 'branch' or 'owner:branch'",
        ),
        (
            json!({ "repo": "o/r", "title": "T", "head": "ada:a..b" }),
            r"`head` 'a\.\.b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "title": "T", "head": "x", "base": "a b" }),
            r"`base` 'a b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "title": "T", "head": "x", "draft": "yes" }),
            r"`draft` must be true or false",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&w, "pr_open", args).await, pattern);
    }
    assert_eq!(w.calls().len(), 0);
    assert_matches(
        &refused(
            &w,
            "pr_open",
            json!({ "repo": "o/r", "title": "T", "head": "x" }),
        )
        .await,
        r"Could not find the default branch of o/r; pass `base`",
    );
}

/* ------------------------------------------------------------------ pr_read */

#[tokio::test]
async fn pr_read_reports_the_pull_request_in_full() {
    let w = with_token(vec![reply(200, pull(json!({})))]);
    let payload = ok(&w, "pr_read", json!({ "repo": "o/r", "number": 7 })).await;
    let calls = w.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/pulls/7");
    assert_eq!(
        payload,
        json!({
            "summary": "Pull request #7 in o/r is open: Add login (feat/login into main).",
            "number": 7,
            "title": "Add login",
            "state": "open",
            "draft": false,
            "merged": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "author": "ada",
            "head": "feat/login",
            "base": "main",
            "head_sha": SHA,
            "commits": 3,
            "changed_files": 2,
            "additions": 20,
            "deletions": 4,
            "labels": ["feature"],
            "reviewers": ["bob", "team:platform"],
            "html_url": "https://github.com/o/r/pull/7",
            "body": "Adds login",
        })
    );
}

#[tokio::test]
async fn pr_read_draft_merged_and_unknown_mergeability() {
    let w = with_token(vec![
        reply(200, pull(json!({ "draft": true }))),
        reply(200, pull(json!({ "state": "closed", "merged": true }))),
        reply(
            200,
            pull(json!({ "mergeable": null, "mergeable_state": "unknown" })),
        ),
    ]);
    let args = json!({ "repo": "o/r", "number": 7 });
    let draft = ok(&w, "pr_read", args.clone()).await;
    assert_matches(
        draft["summary"].as_str().unwrap(),
        r"is a draft, open: Add login",
    );

    let merged = ok(&w, "pr_read", args.clone()).await;
    assert_matches(merged["summary"].as_str().unwrap(), r"is merged: Add login");
    assert_eq!(merged["merged"], true);

    let unknown = ok(&w, "pr_read", args).await;
    assert!(unknown.get("mergeable").is_none());
    assert_matches(
        unknown["summary"].as_str().unwrap(),
        r"Whether it can be merged is still being computed; ask again in a moment\.$",
    );
}

#[tokio::test]
async fn pr_read_lists_the_changed_files_when_asked() {
    let w = with_token(vec![
        reply(200, pull(json!({}))),
        reply(
            200,
            json!([
                { "filename": "a.js", "status": "added", "additions": 10, "deletions": 0, "patch": "@@" },
                { "filename": "b.js", "status": "modified", "additions": 10, "deletions": 4 },
            ]),
        ),
    ]);
    let payload = ok(
        &w,
        "pr_read",
        json!({ "repo": "o/r", "number": 7, "files": true }),
    )
    .await;
    assert_eq!(
        w.calls()[1].url,
        "https://api.github.com/repos/o/r/pulls/7/files?per_page=100"
    );
    assert_eq!(
        payload["files"],
        json!([
            { "filename": "a.js", "status": "added", "additions": 10, "deletions": 0 },
            { "filename": "b.js", "status": "modified", "additions": 10, "deletions": 4 },
        ])
    );
}

#[tokio::test]
async fn pr_read_reports_a_missing_pull_request_and_refuses_bad_arguments() {
    let w = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(200, json!({ "nope": 1 })),
    ]);
    let args = json!({ "repo": "o/r", "number": 9 });
    assert_matches(
        &refused(&w, "pr_read", args.clone()).await,
        r"could not find what it needs to read pull request #9 of o/r \(404\)",
    );
    assert_matches(
        &refused(&w, "pr_read", args).await,
        r"the body was not a pull request this server recognizes",
    );
    assert_eq!(w.calls().len(), 2);
    assert_matches(
        &refused(&w, "pr_read", json!({ "repo": "o/r" })).await,
        r"`number` is required",
    );
    assert_matches(
        &refused(
            &w,
            "pr_read",
            json!({ "repo": "o/r", "number": 7, "files": 1 }),
        )
        .await,
        r"`files` must be true or false",
    );
    assert_eq!(w.calls().len(), 2);
}

/* ------------------------------------------------------------------ pr_comment */

#[tokio::test]
async fn pr_comment_posts_through_the_issue_comments_endpoint() {
    let w = with_token(vec![reply(
        201,
        json!({ "id": 5, "html_url": "https://github.com/o/r/pull/7#issuecomment-5" }),
    )]);
    let payload = ok(
        &w,
        "pr_comment",
        json!({ "repo": "o/r", "number": 7, "body": "Looks good\n" }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/7/comments"
    );
    assert_eq!(calls[0].body, json!({ "body": "Looks good\n" }));
    assert_eq!(
        payload,
        json!({ "summary": "Commented on pull request #7 in o/r.", "id": 5, "html_url": "https://github.com/o/r/pull/7#issuecomment-5" })
    );
}

#[tokio::test]
async fn pr_comment_says_when_locked_and_refuses_a_blank_body() {
    let w = with_token(vec![reply(
        403,
        json!({ "message": "Unable to create comment because issue is locked." }),
    )]);
    assert_matches(
        &refused(
            &w,
            "pr_comment",
            json!({ "repo": "o/r", "number": 7, "body": "x" }),
        )
        .await,
        r"The conversation is locked\.$",
    );
    assert_matches(
        &refused(
            &w,
            "pr_comment",
            json!({ "repo": "o/r", "number": 7, "body": " " }),
        )
        .await,
        r"`body` is required and must not be blank",
    );
    assert_matches(
        &refused(&w, "pr_comment", json!({ "repo": "o/r", "body": "x" })).await,
        r"`number` is required",
    );
    assert_eq!(w.calls().len(), 1);
}

/* ------------------------------------------------------------------ pr_close */

#[tokio::test]
async fn pr_close_closes_without_merging() {
    let w = with_token(vec![reply(200, pull(json!({ "state": "closed" })))]);
    let payload = ok(&w, "pr_close", json!({ "repo": "o/r", "number": 7 })).await;
    let calls = w.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PATCH");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/pulls/7");
    assert_eq!(calls[0].body, json!({ "state": "closed" }));
    assert_eq!(
        payload,
        json!({
            "summary": "Closed pull request #7 in o/r without merging.",
            "number": 7,
            "state": "closed",
            "merged": false,
            "html_url": "https://github.com/o/r/pull/7",
        })
    );
}

#[tokio::test]
async fn pr_close_comments_first_and_reports_already_merged() {
    let w = with_token(vec![
        reply(201, json!({ "id": 1, "html_url": "x" })),
        reply(200, pull(json!({ "state": "closed" }))),
        reply(200, pull(json!({ "state": "closed", "merged": true }))),
    ]);
    ok(
        &w,
        "pr_close",
        json!({ "repo": "o/r", "number": 7, "comment": "Superseded by #8" }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/issues/7/comments"
    );
    assert_eq!(calls[0].body, json!({ "body": "Superseded by #8" }));

    let merged = ok(&w, "pr_close", json!({ "repo": "o/r", "number": 7 })).await;
    assert_eq!(
        merged["summary"],
        "Pull request #7 in o/r was already merged; nothing to close."
    );
    assert_eq!(merged["merged"], true);
}

#[tokio::test]
async fn pr_close_says_the_comment_was_posted_when_closing_fails() {
    let w = with_token(vec![
        reply(201, json!({ "id": 1, "html_url": "x" })),
        reply(403, json!({ "message": "Must have push access" })),
    ]);
    assert_matches(
        &refused(
            &w,
            "pr_close",
            json!({ "repo": "o/r", "number": 7, "comment": "Bye" }),
        )
        .await,
        r"close pull request #7 of o/r \(403\).* The comment had already been posted\.$",
    );
    assert_matches(
        &refused(&w, "pr_close", json!({ "repo": "o/r" })).await,
        r"`number` is required",
    );
    assert_matches(
        &refused(
            &w,
            "pr_close",
            json!({ "repo": "o/r", "number": 7, "comment": 3 }),
        )
        .await,
        r"`comment` must be a string",
    );
    assert_eq!(w.calls().len(), 2);
}

/* ------------------------------------------------------------------ pr_merge */

#[tokio::test]
async fn pr_merge_merges_by_merge_commit_by_default() {
    let w = with_token(vec![reply(
        200,
        json!({ "sha": SHA, "merged": true, "message": "Pull Request successfully merged" }),
    )]);
    let payload = ok(&w, "pr_merge", json!({ "repo": "o/r", "number": 7 })).await;
    let calls = w.calls();
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/pulls/7/merge"
    );
    assert_eq!(calls[0].body, json!({ "merge_method": "merge" }));
    assert_eq!(
        payload,
        json!({
            "summary": "Merged pull request #7 in o/r by merge, at 0123456.",
            "merged": true,
            "sha": SHA,
            "message": "Pull Request successfully merged",
        })
    );
}

#[tokio::test]
async fn pr_merge_passes_method_commit_text_and_sha() {
    let w = with_token(vec![reply(200, json!({ "sha": SHA, "merged": true }))]);
    ok(
        &w,
        "pr_merge",
        json!({ "repo": "o/r", "number": 7, "method": "squash", "title": " Add login (#7) ", "message": "Body\n", "sha": SHA }),
    )
    .await;
    assert_eq!(
        w.calls()[0].body,
        json!({ "merge_method": "squash", "commit_title": "Add login (#7)", "commit_message": "Body\n", "sha": SHA })
    );
}

#[tokio::test]
async fn pr_merge_explains_not_mergeable_and_a_moved_branch() {
    let w = with_token(vec![
        reply(405, json!({ "message": "Pull Request is not mergeable" })),
        reply(
            409,
            json!({ "message": "Head branch was modified. Review and try the merge again." }),
        ),
    ]);
    assert_matches(
        &refused(
            &w,
            "pr_merge",
            json!({ "repo": "o/r", "number": 7, "method": "rebase" }),
        )
        .await,
        r"^GitHub would not merge #7 in o/r \(405\): Pull Request is not mergeable\. It is not mergeable: .*rebase merges may be disabled.*merge queue cannot be merged this way\.$",
    );
    assert_matches(
        &refused(
            &w,
            "pr_merge",
            json!({ "repo": "o/r", "number": 7, "sha": SHA }),
        )
        .await,
        r"\(409\): Head branch was modified.*read the pull request again with pr_read\.$",
    );
}

#[tokio::test]
async fn pr_merge_reports_a_200_that_did_not_merge_as_a_failure() {
    let w = with_token(vec![reply(
        200,
        json!({ "merged": false, "message": "Merge queued" }),
    )]);
    let done = call(&w, "pr_merge", json!({ "repo": "o/r", "number": 7 }))
        .await
        .unwrap_or_else(|e| panic!("refused: {}", e.message()));
    assert!(!done.ok);
    assert_eq!(done.payload["merged"], false);
    assert_eq!(
        done.payload["summary"],
        "GitHub did not merge #7 in o/r: Merge queued"
    );
    expect_output(&def("pr_merge"), &done.payload);
}

#[tokio::test]
async fn pr_merge_refuses_bad_arguments_before_any_request() {
    let w = without_token(vec![reply(200, json!({}))]);
    assert_matches(
        &refused(&w, "pr_merge", json!({ "repo": "o/r" })).await,
        r"`number` is required",
    );
    assert_matches(
        &refused(
            &w,
            "pr_merge",
            json!({ "repo": "o/r", "number": 7, "method": "fast-forward" }),
        )
        .await,
        r"`method` must be one of merge, squash, rebase",
    );
    assert_matches(
        &refused(
            &w,
            "pr_merge",
            json!({ "repo": "o/r", "number": 7, "message": 4 }),
        )
        .await,
        r"`message` must be a string",
    );
    assert_eq!(w.calls().len(), 0);
}

/* ------------------------------------------------------------------ pr_review_request */

#[tokio::test]
async fn pr_review_request_asks_people_and_teams() {
    let w = with_token(vec![reply(
        201,
        pull(
            json!({ "requested_reviewers": [{ "login": "bob" }, { "login": "cy" }], "requested_teams": [{ "slug": "platform" }] }),
        ),
    )]);
    let payload = ok(
        &w,
        "pr_review_request",
        json!({ "repo": "o/r", "number": 7, "reviewers": ["bob", "cy"], "team_reviewers": ["platform"] }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/pulls/7/requested_reviewers"
    );
    assert_eq!(
        calls[0].body,
        json!({ "reviewers": ["bob", "cy"], "team_reviewers": ["platform"] })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Review requested on #7 in o/r from: bob, cy, team platform.",
            "number": 7,
            "reviewers": ["bob", "cy"],
            "teams": ["platform"],
        })
    );
}

#[tokio::test]
async fn pr_review_request_sends_only_the_side_given() {
    let w = with_token(vec![
        reply(201, pull(json!({ "requested_teams": [] }))),
        reply(201, pull(json!({ "requested_reviewers": [] }))),
    ]);
    ok(
        &w,
        "pr_review_request",
        json!({ "repo": "o/r", "number": 7, "reviewers": ["bob"] }),
    )
    .await;
    assert_eq!(w.calls()[0].body, json!({ "reviewers": ["bob"] }));
    let teams = ok(
        &w,
        "pr_review_request",
        json!({ "repo": "o/r", "number": 7, "team_reviewers": ["platform"] }),
    )
    .await;
    assert_eq!(w.calls()[1].body, json!({ "team_reviewers": ["platform"] }));
    assert_eq!(teams["reviewers"], json!([]));
}

#[tokio::test]
async fn pr_review_request_needs_a_reviewer_and_explains_a_refused_one() {
    let w = with_token(vec![reply(
        422,
        json!({ "message": "Review cannot be requested from pull request author." }),
    )]);
    assert_matches(
        &refused(
            &w,
            "pr_review_request",
            json!({ "repo": "o/r", "number": 7 }),
        )
        .await,
        r"^Give at least one of `reviewers` and `team_reviewers`\.$",
    );
    assert_matches(
        &refused(
            &w,
            "pr_review_request",
            json!({ "repo": "o/r", "number": 7, "reviewers": [] }),
        )
        .await,
        r"Give at least one",
    );
    assert_matches(
        &refused(
            &w,
            "pr_review_request",
            json!({ "repo": "o/r", "number": 7, "reviewers": "bob" }),
        )
        .await,
        r"`reviewers` must be a list of strings",
    );
    assert_eq!(w.calls().len(), 0);
    assert_matches(
        &refused(
            &w,
            "pr_review_request",
            json!({ "repo": "o/r", "number": 7, "reviewers": ["ada"] }),
        )
        .await,
        r"\(422\): Review cannot be requested from pull request author\.\. Reviewers must be collaborators with read access",
    );
}

/* ------------------------------------------------------------------ pr_approve */

#[tokio::test]
async fn pr_approve_records_an_approving_review() {
    let w = with_token(vec![reply(
        200,
        json!({ "id": 31, "state": "APPROVED", "html_url": "https://github.com/o/r/pull/7#pullrequestreview-31" }),
    )]);
    let payload = ok(&w, "pr_approve", json!({ "repo": "o/r", "number": 7 })).await;
    let calls = w.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/pulls/7/reviews"
    );
    assert_eq!(calls[0].body, json!({ "event": "APPROVE" }));
    assert_eq!(
        payload,
        json!({
            "summary": "Approved pull request #7 in o/r (review state APPROVED).",
            "id": 31,
            "state": "APPROVED",
            "html_url": "https://github.com/o/r/pull/7#pullrequestreview-31",
        })
    );
}

#[tokio::test]
async fn pr_approve_passes_a_message_and_the_head_commit() {
    let w = with_token(vec![reply(200, json!({ "id": 32, "state": "APPROVED" }))]);
    let payload = ok(
        &w,
        "pr_approve",
        json!({ "repo": "o/r", "number": 7, "body": "LGTM", "sha": SHA }),
    )
    .await;
    assert_eq!(
        w.calls()[0].body,
        json!({ "event": "APPROVE", "body": "LGTM", "commit_id": SHA })
    );
    assert!(payload.get("html_url").is_none());
}

#[tokio::test]
async fn pr_approve_explains_self_approval_and_refuses_bad_arguments() {
    let w = with_token(vec![
        reply(
            422,
            json!({ "message": "Unprocessable Entity", "errors": ["Can not approve your own pull request"] }),
        ),
        reply(200, json!({ "nope": 1 })),
    ]);
    let args = json!({ "repo": "o/r", "number": 7 });
    assert_matches(
        &refused(&w, "pr_approve", args.clone()).await,
        r"\(422\): Unprocessable Entity \(Can not approve your own pull request\)\. A person cannot approve their own pull request; it needs a different reviewer\.$",
    );
    assert_matches(
        &refused(&w, "pr_approve", args).await,
        r"the body was not a review this server recognizes",
    );
    assert_eq!(w.calls().len(), 2);
    assert_matches(
        &refused(&w, "pr_approve", json!({ "repo": "o/r" })).await,
        r"`number` is required",
    );
    assert_matches(
        &refused(
            &w,
            "pr_approve",
            json!({ "repo": "o/r", "number": 7, "body": 3 }),
        )
        .await,
        r"`body` must be a string",
    );
    assert_eq!(w.calls().len(), 2);
}
