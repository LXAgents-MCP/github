//! Ported from test/file.test.ts.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use lxagents_github::config::config::{MAX_FILE_CHARS, MAX_LIST_ENTRIES, MAX_WRITE_BYTES};
use lxagents_github::models::address::{FilePath, Repo};
use lxagents_github::services::contents::{blob_sha, contents_path, encode_content};
use serde_json::{Value, json};

use crate::helpers::*;

fn b64(bytes: impl AsRef<[u8]>) -> String {
    STANDARD.encode(bytes)
}

fn repo() -> Repo {
    Repo {
        owner: "o".into(),
        name: "r".into(),
        full: "o/r".into(),
        path: "/repos/o/r".into(),
    }
}

fn fp(path: &str, encoded: &str) -> FilePath {
    FilePath {
        path: path.into(),
        encoded: encoded.into(),
    }
}

/// A file as the contents API returns it: base64 wrapped across lines.
fn file_body(text: &str, extra: Value) -> Value {
    let mut map = obj(json!({
        "type": "file",
        "encoding": "base64",
        "name": "a.txt",
        "path": "a.txt",
        "sha": "blob1",
        "size": text.len(),
        "content": format!("{}\n", b64(text)),
    }));
    map.extend(obj(extra));
    Value::Object(map)
}

fn written(commit: &str, blob: &str) -> Value {
    json!({"content": {"sha": blob}, "commit": {"sha": commit}})
}

fn written_default() -> Value {
    written("c0ffee0000000000000000000000000000000000", "blob2")
}

fn path_of(url: &str) -> &str {
    url.split('?').next().unwrap()
}

fn param(url: &str, name: &str) -> Option<String> {
    let (_, q) = url.split_once('?')?;
    q.split('&').find_map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (k == name).then(|| v.to_string())
    })
}

async fn expect_refusals(world: &World, tool: &str, cases: Vec<(Value, &str)>) {
    for (args, pattern) in cases {
        assert_matches(&refused(world, tool, args).await, pattern);
    }
}

/* ------------------------------------------------------------------ the shared helpers */

#[test]
fn contents_path_addresses_the_root_and_a_path() {
    assert_eq!(contents_path(&repo(), &fp("", "")), "/repos/o/r/contents");
    assert_eq!(
        contents_path(&repo(), &fp("src/a b.js", "src/a%20b.js")),
        "/repos/o/r/contents/src/a%20b.js"
    );
}

#[test]
fn encode_content_is_base64_of_the_utf8_bytes_and_refuses_a_write_over_the_limit() {
    assert_eq!(encode_content("héllo").unwrap(), b64("héllo"));
    assert_eq!(encode_content("").unwrap(), "");
    let err = encode_content(&"x".repeat(MAX_WRITE_BYTES + 1)).unwrap_err();
    assert_matches(err.message(), "over the 1000000-byte limit for one write");
    assert!(encode_content(&"x".repeat(MAX_WRITE_BYTES)).is_ok());
}

#[tokio::test]
async fn blob_sha_reads_the_sha_of_a_file_on_a_branch_and_says_what_to_do_when_there_is_none() {
    let world = with_token(vec![
        reply(200, file_body("x", json!({"sha": "abc"}))),
        reply(404, json!({"message": "Not Found"})),
        reply(200, json!([{}])),
        reply(200, json!({"type": "symlink"})),
    ]);
    let (r, file) = (repo(), fp("a.txt", "a.txt"));
    let ctx = &world.ctx;
    assert_eq!(
        blob_sha(ctx, &r, &file, Some("dev"), "Use file_create.")
            .await
            .unwrap(),
        "abc"
    );
    assert_eq!(param(&world.calls()[0].url, "ref").as_deref(), Some("dev"));
    let e = blob_sha(ctx, &r, &file, Some("dev"), "Use file_create.")
        .await
        .unwrap_err();
    assert_matches(
        e.message(),
        r"^There is no file 'a\.txt' in o/r on dev\. Use file_create\.$",
    );
    let e = blob_sha(ctx, &r, &file, None, "x").await.unwrap_err();
    assert_matches(e.message(), "is a directory in o/r, not a file");
    let e = blob_sha(ctx, &r, &file, None, "x").await.unwrap_err();
    assert_matches(e.message(), "is not a regular file");
}

/* ------------------------------------------------------------------ file_read */

#[tokio::test]
async fn file_read_decodes_the_file_and_reports_its_sha_and_size() {
    let world = with_token(vec![reply(
        200,
        file_body("héllo\nworld\n", json!({"sha": "deadbeef"})),
    )]);
    let payload = ok(
        &world,
        "file_read",
        json!({"repo": "o/r", "path": "/src/a b.js", "ref": "dev"}),
    )
    .await;
    let c = &world.calls()[0];
    assert_eq!(c.method, "GET");
    assert_eq!(
        path_of(&c.url),
        "https://api.github.com/repos/o/r/contents/src/a%20b.js"
    );
    assert_eq!(param(&c.url, "ref").as_deref(), Some("dev"));
    assert_eq!(
        payload,
        json!({
            "summary": "Read src/a b.js (13 bytes) from o/r at dev.",
            "path": "src/a b.js",
            "sha": "deadbeef",
            "size": 13,
            "text": "héllo\nworld\n",
            "truncated": false,
        })
    );
}

#[tokio::test]
async fn file_read_sends_no_ref_by_default_and_reads_an_empty_file() {
    let world = with_token(vec![reply(200, file_body("", json!({"size": 0})))]);
    let payload = ok(
        &world,
        "file_read",
        json!({"repo": "o/r", "path": "empty.txt"}),
    )
    .await;
    assert_eq!(param(&world.calls()[0].url, "ref"), None);
    assert_eq!(payload["text"], "");
    assert_eq!(payload["summary"], "Read empty.txt (0 bytes) from o/r.");
}

#[tokio::test]
async fn file_read_falls_back_to_windows_1252_and_cuts_a_long_file_at_the_limit() {
    let long = "a".repeat(MAX_FILE_CHARS + 50);
    let world = with_token(vec![
        reply(
            200,
            file_body(
                "",
                json!({"content": b64([0x63, 0x61, 0x66, 0xe9]), "size": 4}),
            ),
        ),
        reply(200, file_body(&long, json!({}))),
    ]);
    let one = ok(
        &world,
        "file_read",
        json!({"repo": "o/r", "path": "latin.txt"}),
    )
    .await;
    assert_eq!(one["text"], "café");

    let two = ok(
        &world,
        "file_read",
        json!({"repo": "o/r", "path": "long.txt"}),
    )
    .await;
    assert_eq!(two["truncated"], true);
    assert_eq!(
        two["text"].as_str().unwrap().chars().count(),
        MAX_FILE_CHARS
    );
    assert_eq!(two["size"], MAX_FILE_CHARS + 50);
    assert_matches(
        two["summary"].as_str().unwrap(),
        r", cut at 25000 characters\.$",
    );
}

#[tokio::test]
async fn file_read_refuses_a_binary_file_a_directory_a_link_a_submodule_and_an_unknown_body() {
    let world = with_token(vec![
        reply(
            200,
            file_body("", json!({"content": b64([1, 0, 2]), "size": 3})),
        ),
        reply(200, json!([{"name": "x", "type": "file"}])),
        reply(200, json!({"type": "symlink", "target": "../real.txt"})),
        reply(
            200,
            json!({"type": "submodule", "submodule_git_url": "https://example.com/m.git"}),
        ),
        reply(
            200,
            json!({"type": "file", "encoding": "none", "content": ""}),
        ),
    ]);
    expect_refusals(
        &world,
        "file_read",
        vec![
            (
                json!({"repo": "o/r", "path": "bin"}),
                r"is a binary file \(3 bytes\); this tool reads text",
            ),
            (
                json!({"repo": "o/r", "path": "dir"}),
                r"is a directory in o/r\. Use directory_list",
            ),
            (
                json!({"repo": "o/r", "path": "ln"}),
                r"is a symbolic link to '\.\./real\.txt'\. Read that path instead",
            ),
            (
                json!({"repo": "o/r", "path": "sub"}),
                r"is a submodule of https://example\.com/m\.git",
            ),
            (
                json!({"repo": "o/r", "path": "big"}),
                "the body was not a file this server recognizes",
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn file_read_explains_a_file_over_1_mb_and_a_missing_file_and_refuses_a_bad_path() {
    let world = with_token(vec![
        reply(
            403,
            json!({"message": "This API returns blobs up to 1 MB in size.", "errors": [{"code": "too_large"}]}),
        ),
        reply(404, json!({"message": "Not Found"})),
    ]);
    expect_refusals(
        &world,
        "file_read",
        vec![
            (
                json!({"repo": "o/r", "path": "big.bin"}),
                r"is over the 1 MB GitHub returns through this API \(403\).*Fetch it with git instead",
            ),
            (
                json!({"repo": "o/r", "path": "nope.txt"}),
                r"could not find what it needs to read 'nope\.txt' in o/r \(404\)",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 2);
    expect_refusals(
        &world,
        "file_read",
        vec![
            (json!({"repo": "o/r"}), "`path` is required"),
            (
                json!({"repo": "o/r", "path": "../etc/passwd"}),
                "is not a path inside the repository",
            ),
            (
                json!({"repo": "o/r", "path": "a", "ref": "x y"}),
                "`ref` 'x y' is not a valid",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 2);
}

/* ------------------------------------------------------------------ directory_list */

#[tokio::test]
async fn directory_list_lists_the_root_and_a_subdirectory() {
    let world = with_token(vec![
        reply(
            200,
            json!([{"name": "src", "type": "dir"}, {"name": "README.md", "type": "file", "size": 120}]),
        ),
        reply(
            200,
            json!([{"name": "index.js", "type": "file", "size": 9}]),
        ),
    ]);
    let root = ok(&world, "directory_list", json!({"repo": "o/r"})).await;
    assert_eq!(
        world.calls()[0].url,
        "https://api.github.com/repos/o/r/contents"
    );
    assert_eq!(
        root,
        json!({
            "summary": "2 entries in the root of o/r.",
            "path": "",
            "items": [
                {"name": "src", "type": "dir", "size": 0},
                {"name": "README.md", "type": "file", "size": 120},
            ],
            "count": 2,
            "truncated": false,
        })
    );

    let sub = ok(
        &world,
        "directory_list",
        json!({"repo": "o/r", "path": "/src/tools/", "ref": "dev"}),
    )
    .await;
    let url = world.calls()[1].url.clone();
    assert_eq!(
        path_of(&url),
        "https://api.github.com/repos/o/r/contents/src/tools"
    );
    assert_eq!(param(&url, "ref").as_deref(), Some("dev"));
    assert_eq!(sub["summary"], "1 entry in 'src/tools' of o/r at dev.");
}

#[tokio::test]
async fn directory_list_shows_the_first_entries_of_a_very_large_directory_and_says_so() {
    let entries: Vec<Value> = (0..MAX_LIST_ENTRIES + 25)
        .map(|i| json!({"name": format!("f{i}"), "type": "file", "size": 1}))
        .collect();
    let world = with_token(vec![reply(200, Value::Array(entries))]);
    let payload = ok(
        &world,
        "directory_list",
        json!({"repo": "o/r", "path": "big"}),
    )
    .await;
    assert_eq!(payload["items"].as_array().unwrap().len(), MAX_LIST_ENTRIES);
    assert_eq!(payload["count"], MAX_LIST_ENTRIES + 25);
    assert_eq!(payload["truncated"], true);
    assert_matches(
        payload["summary"].as_str().unwrap(),
        r"; showing the first 500\.$",
    );
}

#[tokio::test]
async fn directory_list_refuses_a_file_reports_a_missing_path_and_refuses_a_bad_path() {
    let world = with_token(vec![
        reply(200, file_body("x", json!({}))),
        reply(404, json!({"message": "Not Found"})),
    ]);
    expect_refusals(
        &world,
        "directory_list",
        vec![
            (
                json!({"repo": "o/r", "path": "a.txt"}),
                r"is a file in o/r, not a directory\. Use file_read",
            ),
            (
                json!({"repo": "o/r", "path": "nope"}),
                r"could not find what it needs to list 'nope' of o/r \(404\)",
            ),
            (
                json!({"repo": "o/r", "path": "a/../b"}),
                "is not a path inside the repository",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 2);
}

/* ------------------------------------------------------------------ file_create */

#[tokio::test]
async fn file_create_commits_the_content_as_base64_and_reports_the_blob_and_the_commit() {
    let world = with_token(vec![reply(
        201,
        written("1234567000000000000000000000000000000000", "blobnew"),
    )]);
    let payload = ok(
        &world,
        "file_create",
        json!({"repo": "o/r", "path": "docs/n.md", "content": "# Notes\n", "message": " docs: add notes ", "branch": "dev"}),
    )
    .await;
    let c = &world.calls()[0];
    assert_eq!(c.method, "PUT");
    assert_eq!(c.url, "https://api.github.com/repos/o/r/contents/docs/n.md");
    assert_eq!(
        c.body,
        json!({"message": "docs: add notes", "content": b64("# Notes\n"), "branch": "dev"})
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Created docs/n.md in o/r on dev (commit 1234567).",
            "path": "docs/n.md",
            "sha": "blobnew",
            "commit_sha": "1234567000000000000000000000000000000000",
            "branch": "dev",
        })
    );
}

#[tokio::test]
async fn file_create_sends_no_branch_by_default_and_creates_an_empty_file() {
    let world = with_token(vec![reply(201, written_default())]);
    let payload = ok(
        &world,
        "file_create",
        json!({"repo": "o/r", "path": "empty", "content": "", "message": "add empty"}),
    )
    .await;
    assert_eq!(
        world.calls()[0].body,
        json!({"message": "add empty", "content": ""})
    );
    assert_matches(
        payload["summary"].as_str().unwrap(),
        r"^Created empty in o/r on the default branch \(commit c0ffee0\)\.$",
    );
    assert!(payload.get("branch").is_none());
}

#[tokio::test]
async fn file_create_points_at_file_update_when_the_file_exists_and_explains_a_moved_branch() {
    let world = with_token(vec![
        reply(
            422,
            json!({"message": "Invalid request.\n\n\"sha\" wasn't supplied."}),
        ),
        reply(409, json!({"message": "is at abc but expected def"})),
    ]);
    expect_refusals(
        &world,
        "file_create",
        vec![
            (
                json!({"repo": "o/r", "path": "a.txt", "content": "x", "message": "m", "branch": "dev"}),
                r"The file already exists on dev; use file_update to change it\.$",
            ),
            (
                json!({"repo": "o/r", "path": "a.txt", "content": "x", "message": "m"}),
                r"\(409\).*The branch moved while the commit was made; try again\.$",
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn file_create_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(201, written_default())]);
    expect_refusals(
        &world,
        "file_create",
        vec![
            (json!({"repo": "o/r", "content": "x", "message": "m"}), "`path` is required"),
            (json!({"repo": "o/r", "path": "a", "message": "m"}), "`content` is required"),
            (
                json!({"repo": "o/r", "path": "a", "content": 5, "message": "m"}),
                "`content` must be a string",
            ),
            (json!({"repo": "o/r", "path": "a", "content": "x"}), "`message` is required"),
            (
                json!({"repo": "o/r", "path": "../a", "content": "x", "message": "m"}),
                "is not a path inside the repository",
            ),
            (
                json!({"repo": "o/r", "path": "a", "content": "x".repeat(MAX_WRITE_BYTES + 1), "message": "m"}),
                "over the 1000000-byte limit",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}

#[tokio::test]
async fn file_create_refuses_an_answer_with_no_commit() {
    let world = with_token(vec![reply(201, json!({"content": {"sha": "x"}}))]);
    assert_matches(
        &refused(
            &world,
            "file_create",
            json!({"repo": "o/r", "path": "a", "content": "x", "message": "m"}),
        )
        .await,
        "the body was not a commit this server recognizes",
    );
}

/* ------------------------------------------------------------------ file_update */

#[tokio::test]
async fn file_update_reads_the_current_sha_then_replaces_the_file() {
    let world = with_token(vec![
        reply(200, file_body("old", json!({"sha": "oldsha"}))),
        reply(
            200,
            written("abcdef1000000000000000000000000000000000", "newsha"),
        ),
    ]);
    let payload = ok(
        &world,
        "file_update",
        json!({"repo": "o/r", "path": "a.txt", "content": "new", "message": "update a", "branch": "dev"}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].method, "GET");
    assert_eq!(param(&calls[0].url, "ref").as_deref(), Some("dev"));
    assert_eq!(calls[1].method, "PUT");
    assert_eq!(
        calls[1].body,
        json!({"message": "update a", "content": b64("new"), "sha": "oldsha", "branch": "dev"})
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Updated a.txt in o/r on dev (commit abcdef1).",
            "path": "a.txt",
            "sha": "newsha",
            "commit_sha": "abcdef1000000000000000000000000000000000",
            "branch": "dev",
        })
    );
}

#[tokio::test]
async fn file_update_uses_a_sha_it_is_given_and_makes_a_single_request() {
    let world = with_token(vec![reply(200, written_default())]);
    ok(
        &world,
        "file_update",
        json!({"repo": "o/r", "path": "a.txt", "content": "new", "message": "m", "sha": " mine "}),
    )
    .await;
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].body["sha"], "mine");
    assert!(calls[0].body.get("branch").is_none());
}

#[tokio::test]
async fn file_update_points_at_file_create_for_a_missing_file_and_says_a_changed_file_must_be_read_again()
 {
    let world = with_token(vec![
        reply(404, json!({"message": "Not Found"})),
        reply(409, json!({"message": "a.txt does not match abc"})),
        reply(422, json!({"message": "sha wasn't supplied"})),
    ]);
    expect_refusals(
        &world,
        "file_update",
        vec![
            (
                json!({"repo": "o/r", "path": "a.txt", "content": "x", "message": "m"}),
                r"^There is no file 'a\.txt' in o/r\. Use file_create to make it\.$",
            ),
            (
                json!({"repo": "o/r", "path": "a.txt", "content": "x", "message": "m", "sha": "stale"}),
                r"\(409\): a\.txt does not match abc\. The file changed since its sha was read; read it again with file_read",
            ),
            (
                json!({"repo": "o/r", "path": "a.txt", "content": "x", "message": "m", "sha": "stale"}),
                r"\(422\).*read it again",
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn file_update_refuses_a_directory_at_the_path_and_bad_arguments_before_any_request() {
    let world = with_token(vec![reply(200, json!([{"name": "x"}]))]);
    assert_matches(
        &refused(
            &world,
            "file_update",
            json!({"repo": "o/r", "path": "dir", "content": "x", "message": "m"}),
        )
        .await,
        "is a directory in o/r, not a file",
    );
    assert_eq!(world.calls().len(), 1);
    expect_refusals(
        &world,
        "file_update",
        vec![
            (
                json!({"repo": "o/r", "path": "a", "message": "m"}),
                "`content` is required",
            ),
            (
                json!({"repo": "o/r", "path": "a", "content": "x"}),
                "`message` is required",
            ),
            (
                json!({"repo": "o/r", "path": "a", "content": "x", "message": "m", "sha": 5}),
                "`sha` must be a string",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 1);
}

/* ------------------------------------------------------------------ file_delete */

#[tokio::test]
async fn file_delete_reads_the_current_sha_then_deletes_with_it() {
    let world = with_token(vec![
        reply(200, file_body("x", json!({"sha": "gone1"}))),
        reply(
            200,
            json!({"content": null, "commit": {"sha": "9876543000000000000000000000000000000000"}}),
        ),
    ]);
    let payload = ok(
        &world,
        "file_delete",
        json!({"repo": "o/r", "path": "old.txt", "message": "remove old", "branch": "dev"}),
    )
    .await;
    let c = &world.calls()[1];
    assert_eq!(c.method, "DELETE");
    assert_eq!(c.url, "https://api.github.com/repos/o/r/contents/old.txt");
    assert_eq!(
        c.body,
        json!({"message": "remove old", "sha": "gone1", "branch": "dev"})
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Deleted old.txt from o/r on dev (commit 9876543).",
            "deleted": true,
            "path": "old.txt",
            "commit_sha": "9876543000000000000000000000000000000000",
        })
    );
}

#[tokio::test]
async fn file_delete_uses_a_sha_it_is_given_and_says_there_is_nothing_to_delete_for_a_missing_file()
{
    let world = with_token(vec![
        reply(200, json!({"commit": {"sha": "c"}})),
        reply(404, json!({"message": "Not Found"})),
        reply(409, json!({"message": "mismatch"})),
    ]);
    call(
        &world,
        "file_delete",
        json!({"repo": "o/r", "path": "a", "message": "m", "sha": "s1"}),
    )
    .await
    .unwrap();
    let calls = world.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].body, json!({"message": "m", "sha": "s1"}));
    expect_refusals(
        &world,
        "file_delete",
        vec![
            (
                json!({"repo": "o/r", "path": "a", "message": "m"}),
                r"^There is no file 'a' in o/r\. There is nothing to delete\.$",
            ),
            (
                json!({"repo": "o/r", "path": "a", "message": "m", "sha": "old"}),
                "The file changed since its sha was read; read it again with file_read before deleting",
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn file_delete_refuses_bad_arguments_before_any_request() {
    let world = without_token(vec![reply(200, json!({}))]);
    expect_refusals(
        &world,
        "file_delete",
        vec![
            (json!({"repo": "o/r", "message": "m"}), "`path` is required"),
            (json!({"repo": "o/r", "path": "a"}), "`message` is required"),
            (
                json!({"repo": "o/r", "path": "./a", "message": "m"}),
                "is not a path inside the repository",
            ),
        ],
    )
    .await;
    assert_eq!(world.calls().len(), 0);
}
