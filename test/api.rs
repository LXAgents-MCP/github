//! Ported from test/api.test.ts.
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::FutureExt;
use lxagents_github::config::config::{API_TIMEOUT_MS, TOKEN_ENV};
use lxagents_github::config::version::VERSION;
use lxagents_github::models::address::{
    FilePath, Repo, Workflow, file_path, parse_repo, ref_name, ref_path, workflow_name,
};
use lxagents_github::models::paging::{PageQuery, page_query, paging};
use lxagents_github::services::api::{GitHubOptions, github};
use lxagents_github::services::auth::require_token;
use lxagents_github::services::client::{ApiAnswer, CallOptions, ReqwestBackend, call_api};
use lxagents_github::services::context::{Ctx, TransportError};
use lxagents_github::services::failure::api_detail;
use lxagents_github::utils::errors::Refused;
use lxagents_github::utils::redact::redact;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::helpers::{
    Answer, TOKEN, TestEnv, assert_matches, obj, reply, reply_with, with_token, without_token,
};

#[track_caller]
fn refuses<T: std::fmt::Debug>(result: Result<T, Refused>, pattern: &str) -> String {
    match result {
        Ok(v) => panic!("did not refuse: {v:?}"),
        Err(e) => {
            assert_matches(e.message(), pattern);
            e.message().to_string()
        }
    }
}

/* ------------------------------------------------------------------ the token */

#[test]
fn require_token_returns_the_trimmed_github_key() {
    let world = without_token(vec![]);
    world.env.set(TOKEN_ENV, "  abc123  ");
    assert_eq!(require_token(&world.ctx).unwrap(), "abc123");
}

#[test]
fn require_token_refuses_a_missing_or_blank_token_and_names_the_variable() {
    let world = without_token(vec![]);
    refuses(
        require_token(&world.ctx),
        r"^GITHUB_API_KEY is not set\. .*`env` block",
    );
    world.env.set(TOKEN_ENV, "   ");
    refuses(require_token(&world.ctx), "GITHUB_API_KEY is not set");
}

#[test]
fn require_token_reads_the_github_variable_by_default() {
    let world = without_token(vec![]);
    refuses(require_token(&world.ctx), r"^GITHUB_API_KEY is not set\.");
}

#[test]
fn redact_removes_every_occurrence_of_a_secret_and_tolerates_an_empty_one() {
    assert_eq!(redact("a s3cret b s3cret", "s3cret"), "a *** b ***");
    assert_eq!(redact("nothing here", "s3cret"), "nothing here");
    assert_eq!(redact("kept", ""), "kept");
}

/* ------------------------------------------------------------------ call_api */

fn get(url: &str) -> (String, CallOptions) {
    (url.to_string(), CallOptions::default())
}

#[tokio::test]
async fn call_api_sends_the_method_headers_and_a_json_body_and_decodes_the_answer() {
    let world = without_token(vec![reply(201, json!("{\"ok\":true}"))]);
    let result = call_api(
        &world.ctx,
        "https://api.example.com/things",
        CallOptions {
            method: "POST".into(),
            headers: vec![("Authorization".into(), "Bearer x".into())],
            json: Some(json!({"a": 1})),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.status, 201);
    assert_eq!(result.body, json!({"ok": true}));
    let call = &world.calls()[0];
    assert_eq!(call.url, "https://api.example.com/things");
    assert_eq!(call.method, "POST");
    assert_eq!(
        call.headers,
        vec![
            ("Authorization".to_string(), "Bearer x".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]
    );
    assert_eq!(call.raw_body.as_deref(), Some(r#"{"a":1}"#));
}

#[tokio::test]
async fn call_api_hands_back_the_response_headers_lowercased() {
    let world = without_token(vec![reply_with(
        200,
        json!("[]"),
        &[("Link", "<x>; rel=\"next\"")],
    )]);
    let (url, options) = get("https://api.example.com/things");
    let answer = call_api(&world.ctx, &url, options).await.unwrap();
    assert_eq!(answer.headers["link"], "<x>; rel=\"next\"");
}

#[tokio::test]
async fn call_api_sends_no_body_and_no_content_type_when_there_is_no_json() {
    let world = without_token(vec![reply(204, Value::Null)]);
    let (url, options) = get("https://api.example.com/things");
    let answer = call_api(&world.ctx, &url, options).await.unwrap();
    assert_eq!(answer.status, 204);
    assert_eq!(answer.body, Value::Null);
    let call = &world.calls()[0];
    assert_eq!(call.method, "GET");
    assert_eq!(call.raw_body, None);
    assert!(call.headers.is_empty());
}

#[tokio::test]
async fn call_api_hands_back_a_body_that_is_not_json_as_short_text() {
    let world = without_token(vec![reply(502, json!("x".repeat(1000)))]);
    let (url, options) = get("https://api.example.com/things");
    let answer = call_api(&world.ctx, &url, options).await.unwrap();
    assert_eq!(answer.status, 502);
    assert_eq!(answer.body, json!("x".repeat(300)));
}

#[tokio::test]
async fn call_api_keeps_the_whole_body_as_text_when_asked_for_it_raw() {
    let world = without_token(vec![reply(200, json!("y".repeat(1000)))]);
    let options = CallOptions {
        raw: true,
        ..Default::default()
    };
    let answer = call_api(&world.ctx, "https://api.example.com/log", options)
        .await
        .unwrap();
    assert_eq!(answer.body, json!("y".repeat(1000)));
}

#[tokio::test]
async fn call_api_still_decodes_an_error_answer_when_a_good_one_is_asked_for_raw() {
    let world = without_token(vec![reply(404, json!({"message": "Not Found"}))]);
    let options = CallOptions {
        raw: true,
        ..Default::default()
    };
    let answer = call_api(&world.ctx, "https://api.example.com/log", options)
        .await
        .unwrap();
    assert_eq!(answer.status, 404);
    assert_eq!(answer.body, json!({"message": "Not Found"}));
}

/// A context on the real network, with its own limits.
fn real(timeout: Duration, max_bytes: usize) -> Ctx {
    Ctx {
        env: Arc::new(TestEnv::default()),
        http: Arc::new(ReqwestBackend::with_limits(timeout, max_bytes)),
        log: Arc::new(|_: &str| {}),
    }
}

/// A one-handler HTTP server on loopback: each connection's request head is handed to `respond`,
/// which returns the bytes to write back (or `None` to keep the connection silent).
async fn serve<F>(respond: F) -> u16
where
    F: Fn(String) -> Option<Vec<u8>> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let respond = Arc::new(respond);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let respond = respond.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut buf = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                match respond(String::from_utf8_lossy(&head).into_owned()) {
                    Some(bytes) => {
                        let _ = socket.write_all(&bytes).await;
                        let _ = socket.shutdown().await;
                    }
                    None => tokio::time::sleep(Duration::from_secs(30)).await,
                }
            });
        }
    });
    port
}

#[tokio::test]
async fn call_api_follows_a_redirect_to_another_host_without_sending_the_token_there() {
    let seen: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let record = seen.clone();
    let target = serve(move |head| {
        let auth = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
            .map(|l| l.to_string());
        record.lock().unwrap().push(auth);
        Some(b"HTTP/1.1 200 OK\r\ncontent-length: 7\r\nconnection: close\r\n\r\nthe log".to_vec())
    })
    .await;
    let origin = serve(move |_| {
        Some(
            format!(
                "HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:{target}/blob\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            )
            .into_bytes(),
        )
    })
    .await;

    let ctx = real(Duration::from_secs(10), 1_000_000);
    let answer = call_api(
        &ctx,
        &format!("http://127.0.0.1:{origin}/logs"),
        CallOptions {
            raw: true,
            headers: vec![("Authorization".into(), "Bearer secret".into())],
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, json!("the log"));
    assert_eq!(*seen.lock().unwrap(), vec![None]);
}

#[tokio::test]
async fn call_api_refuses_an_answer_larger_than_the_client_will_read() {
    let port = serve(|_| {
        let mut bytes =
            b"HTTP/1.1 200 OK\r\ncontent-length: 50000\r\nconnection: close\r\n\r\n".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', 50_000));
        Some(bytes)
    })
    .await;
    let ctx = real(Duration::from_secs(10), 10_000);
    let options = CallOptions {
        raw: true,
        ..Default::default()
    };
    refuses(
        call_api(&ctx, &format!("http://127.0.0.1:{port}/big"), options).await,
        "larger than 20 MB",
    );
    // And the same refusal through the mock's transport error.
    let world = without_token(vec![Answer::Fail(TransportError::TooLarge)]);
    let (url, options) = get("https://api.example.com/big");
    refuses(
        call_api(&world.ctx, &url, options).await,
        "larger than 20 MB",
    );
}

#[tokio::test]
async fn call_api_turns_a_network_failure_into_a_refusal_with_the_secret_scrubbed() {
    let world = without_token(vec![Answer::Fail(TransportError::Network(
        "getaddrinfo ENOTFOUND for s3cret".into(),
    ))]);
    let error = call_api(
        &world.ctx,
        "https://api.example.com/things",
        CallOptions {
            token: "s3cret".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.message(),
        "Could not reach api.example.com: getaddrinfo ENOTFOUND for ***. Check the network connection."
    );
}

#[tokio::test]
async fn call_api_turns_a_timeout_into_a_refusal_that_says_how_long_it_waited() {
    let pattern = format!(
        r"^api\.example\.com did not answer within {} seconds",
        API_TIMEOUT_MS / 1000
    );
    let world = without_token(vec![Answer::Fail(TransportError::Timeout)]);
    let (url, options) = get("https://api.example.com/things");
    refuses(call_api(&world.ctx, &url, options).await, &pattern);

    // Over a real socket that never answers, the client's own timeout is what fires.
    let port = serve(|_| None).await;
    let ctx = real(Duration::from_millis(200), 1_000_000);
    refuses(
        call_api(
            &ctx,
            &format!("http://127.0.0.1:{port}/slow"),
            CallOptions::default(),
        )
        .await,
        &format!(r"^127\.0\.0\.1:{port} did not answer within"),
    );
}

/* ------------------------------------------------------------------ github() */

async fn gh(
    ctx: &Ctx,
    method: &str,
    path: &str,
    options: GitHubOptions<'_>,
) -> Result<ApiAnswer, Refused> {
    github(ctx, method, path, options).await
}

#[tokio::test]
async fn github_sends_the_token_and_the_pinned_headers_builds_the_query_and_returns_the_answer() {
    let world = with_token(vec![reply_with(200, json!({"ok": 1}), &[("x-thing", "1")])]);
    let answer = gh(
        &world.ctx,
        "GET",
        "/repos/o/r/things",
        GitHubOptions::default()
            .query("page", 2)
            .query("per_page", 30)
            .query("sha", "main")
            .query("empty", "")
            .query("none", Value::Null),
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, json!({"ok": 1}));
    assert_eq!(answer.headers["x-thing"], "1");
    let call = &world.calls()[0];
    assert_eq!(
        call.url,
        "https://api.github.com/repos/o/r/things?page=2&per_page=30&sha=main"
    );
    assert_eq!(call.method, "GET");
    assert_eq!(
        call.header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(call.header("Accept"), Some("application/vnd.github+json"));
    assert_eq!(call.header("X-GitHub-Api-Version"), Some("2022-11-28"));
    assert_eq!(
        call.header("User-Agent"),
        Some(format!("lxagents-mcp-github/{VERSION}").as_str())
    );
    assert_eq!(call.header("Content-Type"), None);
}

#[tokio::test]
async fn github_sends_a_json_body_with_its_content_type_and_a_different_accept_when_asked() {
    let world = with_token(vec![reply(201, json!({"id": 1}))]);
    gh(
        &world.ctx,
        "POST",
        "/repos/o/r/things",
        GitHubOptions::default()
            .json(json!({"a": 1}))
            .accept("application/vnd.github.diff"),
    )
    .await
    .unwrap();
    let call = &world.calls()[0];
    assert_eq!(call.body, json!({"a": 1}));
    assert_eq!(call.header("Content-Type"), Some("application/json"));
    assert_eq!(call.header("Accept"), Some("application/vnd.github.diff"));
}

#[tokio::test]
async fn github_with_no_token_refuses_before_it_sends_anything() {
    let world = without_token(vec![reply(200, json!({}))]);
    refuses(
        gh(&world.ctx, "GET", "/user", GitHubOptions::default()).await,
        r"^GITHUB_API_KEY is not set\.",
    );
    assert!(world.calls().is_empty());
}

#[tokio::test]
async fn github_refuses_a_path_that_could_change_which_endpoint_is_called() {
    let world = with_token(vec![reply(200, json!({}))]);
    for path in ["user", "/user?x=1", "/user#frag"] {
        // A bug, not a refusal: it panics.
        let outcome = AssertUnwindSafe(gh(&world.ctx, "GET", path, GitHubOptions::default()))
            .catch_unwind()
            .await;
        assert!(outcome.is_err(), "{path} did not panic");
    }
    assert!(world.calls().is_empty());
}

#[tokio::test]
async fn github_returns_a_status_listed_in_allow_instead_of_refusing() {
    let world = with_token(vec![reply(404, json!({"message": "Not Found"}))]);
    let answer = gh(
        &world.ctx,
        "GET",
        "/repos/o/r/git/ref/tags/v9",
        GitHubOptions::default().allow(&[404]),
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 404);
}

#[tokio::test]
async fn a_401_says_the_token_was_rejected() {
    let world = with_token(vec![reply(401, json!({"message": "Bad credentials"}))]);
    refuses(
        gh(
            &world.ctx,
            "GET",
            "/user",
            GitHubOptions::what("read the user"),
        )
        .await,
        r"^GitHub rejected the token \(401\)\. Check that GITHUB_API_KEY holds a valid token",
    );
}

#[tokio::test]
async fn a_403_says_what_permission_is_missing_with_githubs_own_hint() {
    let world = with_token(vec![reply_with(
        403,
        json!({"message": "Resource not accessible"}),
        &[("x-accepted-github-permissions", "contents=write")],
    )]);
    refuses(
        gh(
            &world.ctx,
            "PUT",
            "/repos/o/r/contents/a",
            GitHubOptions::what("write the file 'a' in o/r"),
        )
        .await,
        r"^GitHub refused to write the file 'a' in o/r \(403\): Resource not accessible\. .*scope.*accepts: contents=write\)\.$",
    );
}

#[tokio::test]
async fn a_403_that_is_a_rate_limit_says_when_to_try_again() {
    let reset = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 600)
        .to_string();
    let world = with_token(vec![reply_with(
        403,
        json!({"message": "API rate limit exceeded"}),
        &[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", &reset),
        ],
    )]);
    refuses(
        gh(&world.ctx, "GET", "/user", GitHubOptions::default()).await,
        r"^GitHub is rate limiting this token \(403\): API rate limit exceeded\. The limit resets at .*in about (9|10|11) minutes\.$",
    );
}

#[tokio::test]
async fn a_429_with_retry_after_says_how_long_to_wait() {
    let world = with_token(vec![reply_with(
        429,
        json!({"message": "Slow down"}),
        &[("retry-after", "30")],
    )]);
    refuses(
        gh(&world.ctx, "GET", "/user", GitHubOptions::default()).await,
        r"\(429\): Slow down\. Wait 30 seconds and try again\.$",
    );
}

#[tokio::test]
async fn a_404_says_to_check_the_spelling_and_that_a_private_repository_answers_404() {
    let world = with_token(vec![reply(404, json!({"message": "Not Found"}))]);
    refuses(
        gh(
            &world.ctx,
            "GET",
            "/repos/o/r",
            GitHubOptions::what("read the repository o/r"),
        )
        .await,
        r"^GitHub could not find what it needs to read the repository o/r \(404\): Not Found\. .*private repository the token cannot see\.$",
    );
}

#[tokio::test]
async fn a_409_and_a_422_carry_githubs_reasons() {
    let world = with_token(vec![
        reply(409, json!({"message": "Merge conflict"})),
        reply(
            422,
            json!({"message": "Validation Failed", "errors": [{"message": "name is taken"}, {"code": "invalid"}, "plain"]}),
        ),
    ]);
    refuses(
        gh(&world.ctx, "POST", "/x", GitHubOptions::what("merge")).await,
        r"could not merge \(409\): Merge conflict\. .*read it again",
    );
    refuses(
        gh(&world.ctx, "POST", "/x", GitHubOptions::what("create it")).await,
        r"^GitHub could not create it \(422\): Validation Failed \(name is taken; invalid; plain\)\.$",
    );
}

#[tokio::test]
async fn a_5xx_says_github_failed_and_any_other_status_is_reported_with_its_number() {
    let world = with_token(vec![
        reply(503, json!({"message": "Unavailable"})),
        reply(418, json!({"message": "Teapot"})),
    ]);
    refuses(
        gh(&world.ctx, "GET", "/x", GitHubOptions::what("read x")).await,
        r"^GitHub failed to read x \(503\): Unavailable\. Try again shortly\.$",
    );
    refuses(
        gh(&world.ctx, "GET", "/x", GitHubOptions::what("read x")).await,
        r"^GitHub answered 418: Teapot when asked to read x\.$",
    );
}

#[tokio::test]
async fn a_tools_own_wording_wins_and_none_falls_through_to_the_general_wording() {
    let world = with_token(vec![
        reply(422, json!({"message": "bad"})),
        reply(404, json!({"message": "gone"})),
    ]);
    let explain = |status: u16, detail: &str| -> Option<String> {
        (status == 422).then(|| format!("custom {status}{detail}"))
    };
    refuses(
        gh(
            &world.ctx,
            "POST",
            "/x",
            GitHubOptions::default().explain(&explain),
        )
        .await,
        r"^custom 422: bad$",
    );
    refuses(
        gh(
            &world.ctx,
            "GET",
            "/x",
            GitHubOptions::what("find x").explain(&explain),
        )
        .await,
        r"^GitHub could not find what it needs to find x \(404\)",
    );
}

#[tokio::test]
async fn a_failure_never_carries_the_token_even_when_github_echoes_it() {
    let world = with_token(vec![reply(
        500,
        json!({"message": format!("bad token {TOKEN}")}),
    )]);
    let message = refuses(
        gh(&world.ctx, "GET", "/x", GitHubOptions::default()).await,
        "",
    );
    assert!(!message.contains(TOKEN));
    assert!(message.contains("***"));
}

#[test]
fn api_detail_reads_githubs_message_its_error_list_and_plain_text() {
    assert_eq!(api_detail(&json!({"message": "m"})), ": m");
    assert_eq!(
        api_detail(&json!({"message": "m", "errors": [{"message": "a"}, {"code": "b"}]})),
        ": m (a; b)"
    );
    assert_eq!(api_detail(&json!("plain")), ": plain");
    assert_eq!(api_detail(&json!("")), "");
    assert_eq!(api_detail(&Value::Null), "");
    assert_eq!(api_detail(&json!({"other": 1})), "");
}

/* ------------------------------------------------------------------ addressing */

#[test]
fn parse_repo_reads_owner_name_and_builds_the_api_prefix() {
    assert_eq!(
        parse_repo(&json!(" LXAgents-MCP/github "), "repo").unwrap(),
        Repo {
            owner: "LXAgents-MCP".into(),
            name: "github".into(),
            full: "LXAgents-MCP/github".into(),
            path: "/repos/LXAgents-MCP/github".into(),
        }
    );
    assert_eq!(
        parse_repo(&json!("a/b.c_d-e"), "repo").unwrap().name,
        "b.c_d-e"
    );
    assert_eq!(
        parse_repo(&json!("user_shortcode/x"), "repo")
            .unwrap()
            .owner,
        "user_shortcode"
    );
}

#[test]
fn parse_repo_refuses_anything_that_is_not_exactly_owner_name() {
    for value in [
        "", "  ", "solo", "a/b/c", "a/", "/b", "a/..", "a/.", "../b", "a/b c", "-a/b", "a/b?x=1",
        "a/b#c", "a b/c",
    ] {
        refuses(parse_repo(&json!(value), "repo"), "`repo`");
    }
    refuses(parse_repo(&Value::Null, "repo"), "`repo` is required");
    refuses(
        parse_repo(&json!(5), "repo"),
        "`repo` must be a string, got number",
    );
    refuses(
        parse_repo(&json!("nope"), "target"),
        r"`target` 'nope' is not a repository\. Write it as 'owner/name'",
    );
    refuses(
        parse_repo(&json!(format!("{}/b", "a".repeat(40))), "repo"),
        "is not a repository",
    );
}

#[test]
fn ref_name_accepts_what_git_accepts() {
    for value in [
        "main",
        "feature/login",
        "v1.2.3",
        "HEAD",
        "0123456789abcdef0123456789abcdef01234567",
        "release/1.0.0",
        "ünï",
    ] {
        assert_eq!(ref_name(&json!(value), "ref").unwrap(), value);
    }
    assert_eq!(ref_name(&json!("  main "), "ref").unwrap(), "main");
}

#[test]
fn ref_name_refuses_what_could_change_the_path_or_that_git_itself_refuses() {
    let long = "a".repeat(256);
    for value in [
        "",
        "a..b",
        "a b",
        "-x",
        "a/",
        "/a",
        "a//b",
        ".hidden",
        "a/.hidden",
        "x.lock",
        "a/x.lock",
        "a~1",
        "a^",
        "a:b",
        "a?b",
        "a*b",
        "a[b",
        "a\\b",
        "a@{b",
        "end.",
        "a\nb",
        &long,
    ] {
        assert!(ref_name(&json!(value), "ref").is_err(), "{value:?}");
    }
    refuses(
        ref_name(&json!("a b"), "branch"),
        r"^`branch` 'a b' is not a valid branch, tag or commit name\.$",
    );
}

#[test]
fn ref_path_keeps_the_slashes_and_encodes_everything_else() {
    assert_eq!(ref_path("feature/login"), "feature/login");
    assert_eq!(ref_path("a#b/c d"), "a%23b/c%20d");
    assert_eq!(ref_path("100%"), "100%25");
}

fn fp(path: &str, encoded: &str) -> FilePath {
    FilePath {
        path: path.into(),
        encoded: encoded.into(),
    }
}

#[test]
fn file_path_cleans_a_path_and_encodes_each_segment() {
    assert_eq!(
        file_path(&json!("/src/a b.js/"), "path", false).unwrap(),
        fp("src/a b.js", "src/a%20b.js")
    );
    assert_eq!(
        file_path(&json!("README.md"), "path", false).unwrap(),
        fp("README.md", "README.md")
    );
    assert_eq!(
        file_path(&json!("dir/ünï#.txt"), "path", false).unwrap(),
        fp("dir/ünï#.txt", "dir/%C3%BCn%C3%AF%23.txt")
    );
}

#[test]
fn file_path_treats_the_root_as_valid_only_when_allowed_and_refuses_traversal() {
    assert_eq!(file_path(&Value::Null, "path", true).unwrap(), fp("", ""));
    assert_eq!(file_path(&json!("/"), "path", true).unwrap(), fp("", ""));
    refuses(file_path(&json!(""), "path", false), "`path` is required");
    for value in ["../x", "a/../b", "a/./b", "a//b", "..", "a\u{0}b"] {
        refuses(
            file_path(&json!(value), "path", false),
            "is not a path inside the repository",
        );
    }
    refuses(
        file_path(&json!(5), "path", false),
        "`path` must be a string",
    );
}

/* ------------------------------------------------------------------ paging */

fn wf(name: &str) -> Workflow {
    Workflow {
        name: name.into(),
        encoded: name.into(),
    }
}

#[test]
fn workflow_name_reads_an_id_or_a_file_name_and_drops_a_leading_workflows_path() {
    assert_eq!(
        workflow_name(&json!("ci.yml"), "workflow").unwrap(),
        wf("ci.yml")
    );
    assert_eq!(
        workflow_name(&json!(" 161335 "), "workflow").unwrap(),
        wf("161335")
    );
    assert_eq!(
        workflow_name(&json!(".github/workflows/release.yaml"), "workflow").unwrap(),
        wf("release.yaml")
    );
    assert_eq!(
        workflow_name(&json!("build_and-test.v2.yml"), "w").unwrap(),
        wf("build_and-test.v2.yml")
    );
}

#[test]
fn workflow_name_refuses_what_is_not_one_name() {
    let long = "x".repeat(256);
    for value in [
        "a/b.yml",
        "../x.yml",
        "..",
        ".hidden.yml",
        "a b.yml",
        "a?x=1",
        "a#b",
        "ci.yml/dispatches",
        &long,
    ] {
        refuses(
            workflow_name(&json!(value), "workflow"),
            r"is not a workflow\. Give its numeric id, or the name of its file",
        );
    }
    refuses(
        workflow_name(&Value::Null, "workflow"),
        "`workflow` is required",
    );
    refuses(
        workflow_name(&json!("  "), "workflow"),
        "`workflow` is required",
    );
    refuses(
        workflow_name(&json!(12), "workflow"),
        "`workflow` must be a string",
    );
}

#[test]
fn page_query_defaults_to_the_first_page_of_thirty_and_bounds_what_it_is_given() {
    let pq = |v: Value| page_query(&obj(v));
    assert_eq!(
        pq(json!({})).unwrap(),
        PageQuery {
            page: 1,
            per_page: 30
        }
    );
    assert_eq!(
        pq(json!({"page": 3, "per_page": 100})).unwrap(),
        PageQuery {
            page: 3,
            per_page: 100
        }
    );
    refuses(pq(json!({"page": 0})), "`page` must be between 1 and");
    refuses(
        pq(json!({"per_page": 101})),
        r"`per_page` must be between 1 and 100, got 101\.",
    );
    refuses(pq(json!({"page": "2"})), "`page` must be a whole number");
}

#[test]
fn paging_says_whether_github_has_another_page() {
    let query = PageQuery {
        page: 2,
        per_page: 30,
    };
    let headers = |link: Option<&str>| {
        link.map(|l| [("link".to_string(), l.to_string())].into_iter().collect())
            .unwrap_or_default()
    };
    assert_eq!(
        Value::Object(paging(
            query,
            &headers(Some(
                "<https://api.github.com/x?page=3>; rel=\"next\", <y>; rel=\"last\""
            ))
        )),
        json!({"page": 2, "per_page": 30, "has_more": true})
    );
    assert_eq!(
        paging(
            query,
            &headers(Some("<https://api.github.com/x?page=1>; rel=\"prev\""))
        )["has_more"],
        json!(false)
    );
    assert_eq!(paging(query, &headers(None))["has_more"], json!(false));
}
