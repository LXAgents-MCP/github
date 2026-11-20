//! Ported from test/branch.test.ts.
use serde_json::{Value, json};

use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn query(url: &str) -> Vec<(String, String)> {
    let Some((_, q)) = url.split_once('?') else {
        return vec![];
    };
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn param(url: &str, name: &str) -> Option<String> {
    query(url)
        .into_iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

async fn expect_refusals(world: &World, tool: &str, cases: Vec<(Value, &str)>) {
    for (args, pattern) in cases {
        assert_matches(&refused(world, tool, args).await, pattern);
    }
}

#[tokio::test]
async fn branch_create_finds_the_default_branchs_sha_then_creates_the_ref() {
    let world = with_token(vec![
        reply(200, json!(SHA)),
        reply(201, json!({"ref": "refs/heads/feat/x"})),
    ]);
    let payload = ok(
        &world,
        "branch_create",
        json!({"repo": "o/r", "branch": "feat/x"}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].method, "GET");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/commits/HEAD"
    );
    assert_eq!(
        calls[0].header("Accept"),
        Some("application/vnd.github.sha")
    );
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].url, "https://api.github.com/repos/o/r/git/refs");
    assert_eq!(
        calls[1].body,
        json!({"ref": "refs/heads/feat/x", "sha": SHA})
    );
    assert_eq!(
        payload,
        json!({"summary": "Created branch feat/x at 0123456 in o/r.", "branch": "feat/x", "sha": SHA})
    );
}

#[tokio::test]
async fn branch_create_can_start_from_another_branch_a_tag_or_a_sha() {
    let world = with_token(vec![
        reply(200, json!(format!("{OTHER}\n"))),
        reply(201, json!({})),
    ]);
    let payload = ok(
        &world,
        "branch_create",
        json!({"repo": "o/r", "branch": "hotfix", "from": "release/1.0"}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/commits/release/1.0"
    );
    assert_eq!(calls[1].body["sha"], OTHER);
    assert_eq!(payload["from"], "release/1.0");
    assert_eq!(
        payload["summary"],
        "Created branch hotfix at fedcba9 in o/r, from release/1.0."
    );
}

#[tokio::test]
async fn branch_create_says_when_the_name_is_taken_the_source_is_missing_or_the_repository_is_empty()
 {
    let world = with_token(vec![
        reply(200, json!(SHA)),
        reply(422, json!({"message": "Reference already exists"})),
        reply(404, json!({"message": "No commit found for the ref nope"})),
        reply(409, json!({"message": "Git Repository is empty."})),
    ]);
    assert_matches(
        &refused(
            &world,
            "branch_create",
            json!({"repo": "o/r", "branch": "main"}),
        )
        .await,
        r"^GitHub could not create the branch 'main' \(422\): Reference already exists\. A branch with that name already exists in o/r; pick another name\.$",
    );
    assert_matches(
        &refused(
            &world,
            "branch_create",
            json!({"repo": "o/r", "branch": "x", "from": "nope"}),
        )
        .await,
        r"could not find what it needs to find 'nope' in o/r to branch from \(404\)",
    );
    assert_matches(
        &refused(
            &world,
            "branch_create",
            json!({"repo": "o/r", "branch": "x"}),
        )
        .await,
        r"^o/r is empty, so there is nothing to branch from\. Create its first commit with file_create\.$",
    );
}

#[tokio::test]
async fn branch_create_refuses_an_answer_that_is_not_a_sha_and_bad_arguments() {
    let world = with_token(vec![reply(200, json!("<html>proxy error</html>"))]);
    assert_matches(
        &refused(
            &world,
            "branch_create",
            json!({"repo": "o/r", "branch": "x"}),
        )
        .await,
        "the body was not a commit sha this server recognizes",
    );
    assert_eq!(world.calls().len(), 1);
    expect_refusals(
        &world,
        "branch_create",
        vec![
            (json!({"repo": "o/r"}), "`branch` is required"),
            (
                json!({"repo": "o/r", "branch": "a..b"}),
                r"`branch` 'a\.\.b' is not a valid branch, tag or commit name",
            ),
            (
                json!({"repo": "o/r", "branch": "x", "from": "a b"}),
                "`from` 'a b' is not a valid",
            ),
            (
                json!({"repo": "nope", "branch": "x"}),
                "is not a repository",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 1);
}

#[tokio::test]
async fn branch_read_reports_the_tip_its_message_and_date_and_protection() {
    let world = with_token(vec![reply(
        200,
        json!({
            "name": "feat/x",
            "protected": true,
            "commit": {"sha": SHA, "commit": {"message": "Add x\n\nLonger body", "author": {"date": "2026-10-01T12:00:00Z"}}},
        }),
    )]);
    let payload = ok(
        &world,
        "branch_read",
        json!({"repo": "o/r", "branch": "feat/x"}),
    )
    .await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/branches/feat/x"
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Branch feat/x of o/r is at 0123456 and is protected.",
            "name": "feat/x",
            "sha": SHA,
            "protected": true,
            "message": "Add x",
            "date": "2026-10-01T12:00:00Z",
        })
    );
}

#[tokio::test]
async fn branch_read_leaves_out_what_github_did_not_give_and_refuses_a_missing_branch() {
    let world = with_token(vec![
        reply(
            200,
            json!({"name": "main", "protected": false, "commit": {"sha": SHA}}),
        ),
        reply(404, json!({"message": "Branch not found"})),
    ]);
    let payload = ok(
        &world,
        "branch_read",
        json!({"repo": "o/r", "branch": "main"}),
    )
    .await;
    assert_eq!(payload["summary"], "Branch main of o/r is at 0123456.");
    assert!(payload.get("message").is_none());
    assert!(payload.get("date").is_none());
    assert_matches(
        &refused(
            &world,
            "branch_read",
            json!({"repo": "o/r", "branch": "gone"}),
        )
        .await,
        r"could not find what it needs to read the branch 'gone' of o/r \(404\): Branch not found",
    );
}

#[tokio::test]
async fn branch_read_encodes_odd_characters_and_refuses_a_name_that_is_not_one() {
    let world = with_token(vec![reply(
        200,
        json!({"name": "a#b", "protected": false, "commit": {"sha": SHA}}),
    )]);
    ok(
        &world,
        "branch_read",
        json!({"repo": "o/r", "branch": "a#b"}),
    )
    .await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/branches/a%23b"
    );
    expect_refusals(
        &world,
        "branch_read",
        vec![
            (
                json!({"repo": "o/r", "branch": "../x"}),
                "is not a valid branch, tag or commit name",
            ),
            (json!({"repo": "o/r"}), "`branch` is required"),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 1);
}

#[tokio::test]
async fn branch_list_sends_the_filter_and_the_paging_and_lists_each_branch() {
    let world = with_token(vec![reply_with(
        200,
        json!([
            {"name": "main", "commit": {"sha": SHA}, "protected": true},
            {"name": "dev", "commit": {"sha": OTHER}, "protected": false},
        ]),
        &[(
            "link",
            r#"<https://api.github.com/repositories/1/branches?page=3>; rel="next""#,
        )],
    )]);
    let payload = ok(
        &world,
        "branch_list",
        json!({"repo": "o/r", "protected": true, "page": 2, "per_page": 2}),
    )
    .await;
    let url = &world.calls()[0].url;
    assert_eq!(
        url.split('?').next().unwrap(),
        "https://api.github.com/repos/o/r/branches"
    );
    let mut q = query(url);
    q.sort();
    assert_eq!(
        q,
        vec![
            ("page".to_string(), "2".to_string()),
            ("per_page".to_string(), "2".to_string()),
            ("protected".to_string(), "true".to_string()),
        ]
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 protected branches of o/r on page 2.",
            "items": [
                {"name": "main", "sha": SHA, "protected": true},
                {"name": "dev", "sha": OTHER, "protected": false},
            ],
            "page": 2,
            "per_page": 2,
            "has_more": true,
        })
    );
}

#[tokio::test]
async fn branch_list_sends_no_filter_by_default_sends_false_when_asked_and_reports_an_empty_page() {
    let world = with_token(vec![
        reply(200, json!([])),
        reply(
            200,
            json!([{"name": "x", "commit": {"sha": SHA}, "protected": false}]),
        ),
    ]);
    let empty = ok(&world, "branch_list", json!({"repo": "o/r"})).await;
    assert_eq!(param(&world.calls()[0].url, "protected"), None);
    assert_eq!(empty["summary"], "o/r has no branches on page 1.");
    assert_eq!(empty["items"], json!([]));
    assert_eq!(empty["has_more"], false);

    let one = ok(
        &world,
        "branch_list",
        json!({"repo": "o/r", "protected": false}),
    )
    .await;
    assert_eq!(
        param(&world.calls()[1].url, "protected").as_deref(),
        Some("false")
    );
    assert_eq!(one["summary"], "1 unprotected branch of o/r on page 1.");
}

#[tokio::test]
async fn branch_list_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, json!([]))]);
    expect_refusals(
        &world,
        "branch_list",
        vec![
            (
                json!({"repo": "o/r", "protected": "yes"}),
                "`protected` must be true or false",
            ),
            (
                json!({"repo": "o/r", "page": 0}),
                "`page` must be between 1 and",
            ),
            (json!({}), "`repo` is required"),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn branch_delete_removes_the_ref_and_says_what_it_removed() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(
        &world,
        "branch_delete",
        json!({"repo": "o/r", "branch": "feat/x"}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "DELETE");
    assert_eq!(
        calls[0].url,
        "https://api.github.com/repos/o/r/git/refs/heads/feat/x"
    );
    assert_eq!(
        payload,
        json!({"summary": "Deleted branch feat/x from o/r.", "deleted": true, "branch": "feat/x"})
    );
}

#[tokio::test]
async fn branch_delete_explains_a_422_as_a_missing_branch_or_the_default_branch() {
    let world = with_token(vec![reply(
        422,
        json!({"message": "Reference does not exist"}),
    )]);
    assert_matches(
        &refused(
            &world,
            "branch_delete",
            json!({"repo": "o/r", "branch": "main"}),
        )
        .await,
        r"^GitHub would not delete the branch 'main' from o/r \(422\): Reference does not exist\. The branch may not exist, or it is the default branch",
    );
}

#[tokio::test]
async fn branch_delete_refuses_bad_arguments_before_any_request() {
    let world = with_token(vec![reply(204, none())]);
    expect_refusals(
        &world,
        "branch_delete",
        vec![
            (json!({"repo": "o/r"}), "`branch` is required"),
            (json!({"repo": "o/r", "branch": "a/../b"}), "is not a valid"),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn branch_merge_merges_the_head_into_the_base_and_reports_the_merge_commit() {
    let world = with_token(vec![reply(
        201,
        json!({"sha": SHA, "commit": {"message": "Merge feat/x into main\n\nbody"}}),
    )]);
    let payload = ok(
        &world,
        "branch_merge",
        json!({"repo": "o/r", "base": "main", "head": "feat/x", "message": " Ship it "}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].url, "https://api.github.com/repos/o/r/merges");
    assert_eq!(
        calls[0].body,
        json!({"base": "main", "head": "feat/x", "commit_message": "Ship it"})
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Merged feat/x into main in o/r, at 0123456.",
            "merged": true,
            "up_to_date": false,
            "sha": SHA,
            "message": "Merge feat/x into main",
        })
    );
}

#[tokio::test]
async fn branch_merge_sends_no_message_by_default_and_treats_a_204_as_nothing_to_merge() {
    let world = with_token(vec![reply(204, none())]);
    let payload = ok(
        &world,
        "branch_merge",
        json!({"repo": "o/r", "base": "main", "head": "feat/x"}),
    )
    .await;
    assert_eq!(
        world.calls()[0].body,
        json!({"base": "main", "head": "feat/x"})
    );
    assert_eq!(
        payload,
        json!({"summary": "Nothing to merge: main already contains feat/x.", "merged": false, "up_to_date": true})
    );
}

#[tokio::test]
async fn branch_merge_explains_a_conflict_and_a_missing_branch() {
    let world = with_token(vec![
        reply(409, json!({"message": "Merge conflict"})),
        reply(404, json!({"message": "Not Found"})),
    ]);
    assert_matches(
        &refused(
            &world,
            "branch_merge",
            json!({"repo": "o/r", "base": "main", "head": "feat/x"}),
        )
        .await,
        r"^GitHub could not merge 'feat/x' into 'main' in o/r \(409\): Merge conflict\. The branches conflict\. Open a pull request",
    );
    assert_matches(
        &refused(
            &world,
            "branch_merge",
            json!({"repo": "o/r", "base": "main", "head": "ghost"}),
        )
        .await,
        r"^GitHub could not find 'main' or 'ghost' in o/r \(404\): Not Found\. Check both names",
    );
}

#[tokio::test]
async fn branch_merge_refuses_bad_arguments_before_any_request() {
    let world = with_token(vec![reply(201, json!({}))]);
    expect_refusals(
        &world,
        "branch_merge",
        vec![
            (json!({"repo": "o/r", "head": "x"}), "`base` is required"),
            (json!({"repo": "o/r", "base": "main"}), "`head` is required"),
            (
                json!({"repo": "o/r", "base": "a b", "head": "x"}),
                "`base` 'a b' is not a valid",
            ),
            (
                json!({"repo": "o/r", "base": "main", "head": "x", "message": 3}),
                "`message` must be a string",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}
