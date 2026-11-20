//! Ported from test/workflow.test.ts.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const BASE: &str = "https://api.github.com/repos/o/r";

/// Merge `overrides` over `base`.
fn merged(mut base: Value, overrides: Value) -> Value {
    if let (Some(map), Value::Object(extra)) = (base.as_object_mut(), overrides) {
        map.extend(extra);
    }
    base
}

/// A URL's path, and its query decoded into a map.
fn split_url(url: &str) -> (String, BTreeMap<String, String>) {
    let rest = url.strip_prefix("https://api.github.com").unwrap_or(url);
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let decode = |s: &str| {
        let bytes = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else if bytes[i] == b'+' {
                out.push(b' ');
                i += 1;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    };
    let params = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect();
    (path.to_string(), params)
}

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// A workflow as GitHub returns it.
fn workflow(overrides: Value) -> Value {
    merged(
        json!({ "id": 161335, "name": "CI", "path": ".github/workflows/ci.yml", "state": "active" }),
        overrides,
    )
}

/// A workflow run as GitHub returns it.
fn run(overrides: Value) -> Value {
    merged(
        json!({
            "id": 77,
            "name": "CI",
            "display_title": "Add login",
            "head_branch": "main",
            "head_sha": SHA,
            "event": "push",
            "status": "completed",
            "conclusion": "failure",
            "run_number": 12,
            "run_attempt": 1,
            "actor": { "login": "ada" },
            "created_at": "2026-10-10T10:00:00Z",
            "updated_at": "2026-10-10T10:05:00Z",
            "html_url": "https://github.com/o/r/actions/runs/77",
        }),
        overrides,
    )
}

/// A job as GitHub returns it.
fn job(overrides: Value) -> Value {
    merged(
        json!({
            "id": 1,
            "name": "build",
            "status": "completed",
            "conclusion": "success",
            "steps": [{ "name": "Set up job", "status": "completed", "conclusion": "success", "number": 1 }],
        }),
        overrides,
    )
}

fn failed_job(id: u64, name: Option<&str>) -> Value {
    let name = name.map_or_else(|| format!("job-{id}"), str::to_string);
    job(json!({
        "id": id,
        "name": name,
        "conclusion": "failure",
        "steps": [
            { "name": "Set up job", "status": "completed", "conclusion": "success", "number": 1 },
            { "name": "Run tests", "status": "completed", "conclusion": "failure", "number": 2 },
            { "name": "Post job cleanup", "status": "completed", "conclusion": "success", "number": 3 },
        ],
    }))
}

/* ------------------------------------------------------------------ workflow_list */

#[tokio::test]
async fn workflow_list_reports_each_workflow_and_whether_more_follow() {
    let w = with_token(vec![reply_with(
        200,
        json!({
            "total_count": 3,
            "workflows": [
                workflow(json!({})),
                workflow(json!({ "id": 2, "name": "Release", "path": ".github/workflows/release.yml", "state": "disabled_manually" })),
            ],
        }),
        &[(
            "link",
            r#"<https://api.github.com/repositories/1/actions/workflows?page=2>; rel="next""#,
        )],
    )]);
    let payload = ok(&w, "workflow_list", json!({ "repo": "o/r", "per_page": 2 })).await;
    let calls = w.calls();
    assert_eq!(calls[0].method, "GET");
    let (path, query) = split_url(&calls[0].url);
    assert_eq!(path, "/repos/o/r/actions/workflows");
    assert_eq!(query, params(&[("page", "1"), ("per_page", "2")]));
    assert_eq!(
        payload,
        json!({
            "summary": "2 of 3 workflows of o/r, on page 1.",
            "items": [
                { "id": 161335, "name": "CI", "path": ".github/workflows/ci.yml", "state": "active" },
                { "id": 2, "name": "Release", "path": ".github/workflows/release.yml", "state": "disabled_manually" },
            ],
            "total_count": 3,
            "page": 1,
            "per_page": 2,
            "has_more": true,
        })
    );
}

#[tokio::test]
async fn workflow_list_says_when_there_are_none_and_uses_the_singular() {
    let w = with_token(vec![
        reply(200, json!({ "total_count": 0, "workflows": [] })),
        reply(
            200,
            json!({ "total_count": 1, "workflows": [workflow(json!({}))] }),
        ),
    ]);
    let none = ok(&w, "workflow_list", json!({ "repo": "o/r" })).await;
    assert_eq!(none["summary"], "o/r has no workflows on page 1.");
    assert_eq!(none["has_more"], false);
    let one = ok(&w, "workflow_list", json!({ "repo": "o/r" })).await;
    assert_eq!(one["summary"], "1 of 1 workflow of o/r, on page 1.");
}

#[tokio::test]
async fn workflow_list_refuses_bad_arguments_before_any_request() {
    let w = with_token(vec![]);
    assert_matches(
        &refused(&w, "workflow_list", json!({ "repo": "nope" })).await,
        r"is not a repository",
    );
    assert_matches(
        &refused(
            &w,
            "workflow_list",
            json!({ "repo": "o/r", "per_page": 101 }),
        )
        .await,
        r"`per_page` must be between 1 and 100",
    );
    assert_matches(
        &refused(&w, "workflow_list", json!({ "repo": "o/r", "page": 0 })).await,
        r"`page` must be between 1",
    );
    assert_eq!(w.calls().len(), 0);
}

/* ------------------------------------------------------------------ workflow_run */

#[tokio::test]
async fn workflow_run_starts_on_the_default_branch_and_reports_the_run() {
    let w = with_token(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(
            200,
            json!({
                "workflow_run_id": 42,
                "run_url": "https://api.github.com/repos/o/r/actions/runs/42",
                "html_url": "https://github.com/o/r/actions/runs/42",
            }),
        ),
    ]);
    let payload = ok(
        &w,
        "workflow_run",
        json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": { "level": "debug", "retries": 3, "dry": true } }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, BASE);
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].url,
        format!("{BASE}/actions/workflows/ci.yml/dispatches")
    );
    assert_eq!(
        calls[1].body,
        json!({
            "ref": "main",
            "inputs": { "level": "debug", "retries": "3", "dry": "true" },
            "return_run_details": true,
        })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Started workflow 'ci.yml' on main in o/r: run 42.",
            "dispatched": true,
            "workflow": "ci.yml",
            "ref": "main",
            "run_id": 42,
            "html_url": "https://github.com/o/r/actions/runs/42",
        })
    );
}

#[tokio::test]
async fn workflow_run_works_when_github_does_not_name_the_run() {
    let w = with_token(vec![reply(204, none()), reply(200, json!({}))]);
    let empty = ok(
        &w,
        "workflow_run",
        json!({ "repo": "o/r", "workflow": "161335", "ref": "release/1.0" }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(
        calls.len(),
        1,
        "a ref that is given needs no lookup of the default branch"
    );
    assert_eq!(
        calls[0].url,
        format!("{BASE}/actions/workflows/161335/dispatches")
    );
    assert_eq!(
        calls[0].body,
        json!({ "ref": "release/1.0", "return_run_details": true })
    );
    assert_eq!(
        empty,
        json!({
            "summary": "Asked GitHub to run workflow '161335' on release/1.0 in o/r. GitHub did not say which run it started; find it with workflow_status in a few seconds.",
            "dispatched": true,
            "workflow": "161335",
            "ref": "release/1.0",
        })
    );
    let bare = ok(
        &w,
        "workflow_run",
        json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "main" }),
    )
    .await;
    assert!(bare.get("run_id").is_none());
    assert_eq!(bare["dispatched"], true);
}

#[tokio::test]
async fn workflow_run_takes_a_file_name_a_path_or_an_id() {
    let w = with_token(vec![
        reply(204, none()),
        reply(204, none()),
        reply(204, none()),
    ]);
    for (workflow, r#ref) in [
        (".github/workflows/release.yaml", "main"),
        (" 161335 ", "main"),
        ("build_and-test.v2.yml", "v1.2.3"),
    ] {
        ok(
            &w,
            "workflow_run",
            json!({ "repo": "o/r", "workflow": workflow, "ref": r#ref }),
        )
        .await;
    }
    let urls: Vec<String> = w.calls().into_iter().map(|c| c.url).collect();
    assert_eq!(
        urls,
        vec![
            format!("{BASE}/actions/workflows/release.yaml/dispatches"),
            format!("{BASE}/actions/workflows/161335/dispatches"),
            format!("{BASE}/actions/workflows/build_and-test.v2.yml/dispatches"),
        ]
    );
}

#[tokio::test]
async fn workflow_run_sends_no_inputs_when_there_are_none() {
    let w = with_token(vec![reply(204, none()), reply(204, none())]);
    ok(
        &w,
        "workflow_run",
        json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "main", "inputs": {} }),
    )
    .await;
    ok(
        &w,
        "workflow_run",
        json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "main", "inputs": null }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(
        calls[0].body,
        json!({ "ref": "main", "return_run_details": true })
    );
    assert_eq!(
        calls[1].body,
        json!({ "ref": "main", "return_run_details": true })
    );
}

#[tokio::test]
async fn workflow_run_refuses_bad_arguments_and_a_missing_default_branch() {
    let w = with_token(vec![reply(200, json!({}))]);
    let many: serde_json::Map<String, Value> =
        (0..26).map(|i| (format!("k{i}"), json!("v"))).collect();
    let cases = [
        (json!({ "repo": "o/r" }), r"`workflow` is required"),
        (
            json!({ "repo": "o/r", "workflow": "a/b.yml" }),
            r"is not a workflow",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "a b" }),
            r"`ref` 'a b' is not a valid",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": "x=1" }),
            r"`inputs` must be an object",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": ["x"] }),
            r"`inputs` must be an object",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": { "a": { "nested": 1 } } }),
            r"`inputs\.a` must be a string, a number or true/false",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": { "a": null } }),
            r"`inputs\.a` must be a string, a number or true/false",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": { " ": "x" } }),
            r"blank name",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "inputs": many }),
            r"at most 25 inputs, got 26",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&w, "workflow_run", args).await, pattern);
    }
    assert_eq!(w.calls().len(), 0);
    assert_matches(
        &refused(
            &w,
            "workflow_run",
            json!({ "repo": "o/r", "workflow": "ci.yml" }),
        )
        .await,
        r"Could not find the default branch of o/r; pass `ref`",
    );
}

#[tokio::test]
async fn workflow_run_explains_a_missing_and_an_undispatchable_workflow() {
    let w = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(
            422,
            json!({ "message": "Workflow does not have 'workflow_dispatch' trigger" }),
        ),
        reply_with(
            403,
            json!({ "message": "Resource not accessible by personal access token" }),
            &[("x-accepted-github-permissions", "actions=write")],
        ),
    ]);
    assert_matches(
        &refused(
            &w,
            "workflow_run",
            json!({ "repo": "o/r", "workflow": "nope.yml", "ref": "main" }),
        )
        .await,
        r"could not find the workflow 'nope\.yml' in o/r \(404\).*Check it with workflow_list.*default branch",
    );
    assert_matches(
        &refused(
            &w,
            "workflow_run",
            json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "main" }),
        )
        .await,
        r"would not start 'ci\.yml' on 'main' in o/r \(422\).*does not have 'workflow_dispatch' trigger.*on: workflow_dispatch",
    );
    assert_matches(
        &refused(
            &w,
            "workflow_run",
            json!({ "repo": "o/r", "workflow": "ci.yml", "ref": "main" }),
        )
        .await,
        r"refused to run the workflow 'ci\.yml' of o/r on 'main' \(403\).*actions=write",
    );
}

/* ------------------------------------------------------------------ workflow_status */

#[tokio::test]
async fn workflow_status_reads_a_run_with_its_jobs_and_failed_step() {
    let w = with_token(vec![
        reply(200, run(json!({}))),
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), failed_job(2, Some("test"))] }),
        ),
    ]);
    let payload = ok(
        &w,
        "workflow_status",
        json!({ "repo": "o/r", "run_id": 77 }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, format!("{BASE}/actions/runs/77"));
    assert_eq!(
        calls[1].url,
        format!("{BASE}/actions/runs/77/jobs?per_page=100")
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Workflow run 77 (CI) on main at 0123456 in o/r is completed, failure. Failed: test.",
            "found": true,
            "run_id": 77,
            "name": "CI",
            "status": "completed",
            "conclusion": "failure",
            "branch": "main",
            "sha": SHA,
            "event": "push",
            "run_number": 12,
            "attempt": 1,
            "actor": "ada",
            "created_at": "2026-10-10T10:00:00Z",
            "updated_at": "2026-10-10T10:05:00Z",
            "html_url": "https://github.com/o/r/actions/runs/77",
            "job_count": 2,
            "jobs": [
                { "id": 1, "name": "build", "status": "completed", "conclusion": "success" },
                { "id": 2, "name": "test", "status": "completed", "conclusion": "failure", "failed_step": "Run tests" },
            ],
        })
    );
}

#[tokio::test]
async fn workflow_status_finds_the_latest_run_on_a_branch_while_running() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 5, "workflow_runs": [run(json!({ "status": "in_progress", "conclusion": null }))] }),
        ),
        reply(
            200,
            json!({ "total_count": 1, "jobs": [job(json!({ "status": "in_progress", "conclusion": null }))] }),
        ),
    ]);
    let payload = ok(
        &w,
        "workflow_status",
        json!({ "repo": "o/r", "workflow": "ci.yml", "branch": "release/1.0" }),
    )
    .await;
    let calls = w.calls();
    let (path, query) = split_url(&calls[0].url);
    assert_eq!(path, "/repos/o/r/actions/workflows/ci.yml/runs");
    assert_eq!(
        query,
        params(&[("per_page", "1"), ("branch", "release/1.0")])
    );
    assert_eq!(
        calls[1].url,
        format!("{BASE}/actions/runs/77/jobs?per_page=100")
    );
    assert_eq!(
        payload["summary"],
        "Workflow run 77 (CI) on main at 0123456 in o/r is in_progress."
    );
    assert!(payload.get("conclusion").is_none());
    assert_eq!(
        payload["jobs"],
        json!([{ "id": 1, "name": "build", "status": "in_progress" }])
    );
}

#[tokio::test]
async fn workflow_status_without_a_branch_asks_for_any_branch() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "workflow_runs": [run(json!({ "conclusion": "success" }))] }),
        ),
        reply(200, json!({ "total_count": 0, "jobs": [] })),
    ]);
    let payload = ok(
        &w,
        "workflow_status",
        json!({ "repo": "o/r", "workflow": "161335" }),
    )
    .await;
    let (path, query) = split_url(&w.calls()[0].url);
    assert_eq!(query, params(&[("per_page", "1")]));
    assert_eq!(path, "/repos/o/r/actions/workflows/161335/runs");
    assert_eq!(
        payload["summary"],
        "Workflow run 77 (CI) on main at 0123456 in o/r is completed, success."
    );
    assert_eq!(payload["job_count"], 0);
    assert_eq!(payload["jobs"], json!([]));
}

#[tokio::test]
async fn workflow_status_says_there_is_no_run_yet_as_an_answer() {
    let w = with_token(vec![
        reply(200, json!({ "total_count": 0, "workflow_runs": [] })),
        reply(200, json!({ "total_count": 0, "workflow_runs": [] })),
    ]);
    let any = call(
        &w,
        "workflow_status",
        json!({ "repo": "o/r", "workflow": "ci.yml" }),
    )
    .await
    .unwrap_or_else(|e| panic!("refused: {}", e.message()));
    assert_eq!(
        any.payload,
        json!({
            "summary": "No run of workflow 'ci.yml' in o/r yet. A run appears a few seconds after it is triggered; ask again shortly.",
            "found": false,
            "jobs": [],
        })
    );
    assert!(any.ok);
    expect_output(&def("workflow_status"), &any.payload);

    let on_branch = ok(
        &w,
        "workflow_status",
        json!({ "repo": "o/r", "workflow": "ci.yml", "branch": "dev" }),
    )
    .await;
    assert_matches(
        on_branch["summary"].as_str().unwrap(),
        r"^No run of workflow 'ci\.yml' on dev in o/r yet\.",
    );
    assert_eq!(
        w.calls().len(),
        2,
        "no jobs are asked for when there is no run"
    );
}

#[tokio::test]
async fn workflow_status_refuses_an_unclear_question_before_any_request() {
    let w = with_token(vec![]);
    let cases = [
        (
            json!({ "repo": "o/r" }),
            r"Give `run_id`, or `workflow` to read its latest run",
        ),
        (
            json!({ "repo": "o/r", "run_id": 1, "workflow": "ci.yml" }),
            r"either `run_id` or `workflow`, not both",
        ),
        (
            json!({ "repo": "o/r", "run_id": 1, "branch": "main" }),
            r"`branch` only narrows the search",
        ),
        (
            json!({ "repo": "o/r", "run_id": 0 }),
            r"`run_id` must be between 1",
        ),
        (
            json!({ "repo": "o/r", "workflow": "a/b" }),
            r"is not a workflow",
        ),
        (
            json!({ "repo": "o/r", "workflow": "ci.yml", "branch": "a b" }),
            r"`branch` 'a b' is not a valid",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&w, "workflow_status", args).await, pattern);
    }
    assert_eq!(w.calls().len(), 0);
}

#[tokio::test]
async fn workflow_status_says_which_run_or_workflow_it_could_not_find() {
    let w = with_token(vec![
        reply(404, json!({ "message": "Not Found" })),
        reply(404, json!({ "message": "Not Found" })),
    ]);
    assert_matches(
        &refused(&w, "workflow_status", json!({ "repo": "o/r", "run_id": 9 })).await,
        r"could not find workflow run 9 in o/r \(404\).*Give a workflow to read its latest run",
    );
    assert_matches(
        &refused(
            &w,
            "workflow_status",
            json!({ "repo": "o/r", "workflow": "nope.yml" }),
        )
        .await,
        r"could not find the workflow 'nope\.yml' in o/r \(404\).*workflow_list",
    );
}

#[tokio::test]
async fn workflow_status_refuses_a_body_that_is_not_a_run() {
    let w = with_token(vec![reply(200, json!({ "message": "hello" }))]);
    assert_matches(
        &refused(
            &w,
            "workflow_status",
            json!({ "repo": "o/r", "run_id": 77 }),
        )
        .await,
        r"not a workflow run this server recognizes",
    );
}

/* ------------------------------------------------------------------ workflow_logs */

const RAW_LOG: &str = "\u{feff}2026-10-10T10:00:00.0000000Z ##[group]Run npm test\r\n\
2026-10-10T10:00:01.0000000Z \u{1b}[31;1mError: boom\u{1b}[0m\r\n\
2026-10-10T10:00:02.0000000Z ##[error]Process completed with exit code 1.\r\n";

#[tokio::test]
async fn workflow_logs_reads_failed_jobs_cleaned() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), failed_job(2, Some("test"))] }),
        ),
        reply(200, json!(RAW_LOG)),
    ]);
    let payload = ok(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 77 })).await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2, "the passing job's log is not fetched");
    assert_eq!(
        calls[0].url,
        format!("{BASE}/actions/runs/77/jobs?per_page=100")
    );
    assert_eq!(calls[1].url, format!("{BASE}/actions/jobs/2/logs"));
    assert_eq!(
        calls[1].header("Accept"),
        Some("application/vnd.github+json")
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Logs of 1 failed job in run 77 of o/r.",
            "run_id": 77,
            "job_count": 1,
            "jobs": [{
                "id": 2,
                "name": "test",
                "status": "completed",
                "conclusion": "failure",
                "failed_step": "Run tests",
                "log": "2026-10-10T10:00:00.0000000Z ##[group]Run npm test\n2026-10-10T10:00:01.0000000Z Error: boom\n2026-10-10T10:00:02.0000000Z ##[error]Process completed with exit code 1.",
                "truncated": false,
            }],
            "truncated": false,
        })
    );
}

#[tokio::test]
async fn workflow_logs_shares_its_budget_and_keeps_the_end() {
    let long = |label: &str| format!("{}\n##[error]{label} failed", "x".repeat(30_000));
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 2, "jobs": [failed_job(2, Some("unit")), failed_job(3, Some("e2e"))] }),
        ),
        reply(200, json!(long("unit"))),
        reply(200, json!(long("e2e"))),
    ]);
    let payload = ok(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 77 })).await;
    assert_eq!(w.calls().len(), 3);
    assert_eq!(
        payload["summary"],
        "Logs of 2 failed jobs in run 77 of o/r. A long log is cut to its last 10000 characters."
    );
    assert_eq!(payload["truncated"], true);
    for (index, label) in ["unit", "e2e"].iter().enumerate() {
        let entry = &payload["jobs"][index];
        let log = entry["log"].as_str().unwrap();
        assert_eq!(entry["truncated"], true);
        assert!(log.starts_with("(truncated) ...\n"));
        assert!(log.ends_with(&format!("##[error]{label} failed")));
        assert_eq!(
            log.chars().count(),
            "(truncated) ...\n".chars().count() + 10_000
        );
    }
}

#[tokio::test]
async fn workflow_logs_reads_at_most_five_jobs() {
    let ids = [1u64, 2, 3, 4, 5, 6, 7];
    let mut answers = vec![reply(
        200,
        json!({ "total_count": 7, "jobs": ids.iter().map(|&id| failed_job(id, None)).collect::<Vec<_>>() }),
    )];
    answers.extend(
        ids[..5]
            .iter()
            .map(|id| reply(200, json!(format!("log of {id}")))),
    );
    let w = with_token(answers);
    let payload = ok(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 77 })).await;
    assert_eq!(w.calls().len(), 6);
    let got: Vec<Value> = payload["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].clone())
        .collect();
    assert_eq!(got, vec![json!(1), json!(2), json!(3), json!(4), json!(5)]);
    assert_eq!(payload["job_count"], 7);
    assert_eq!(payload["truncated"], true);
    assert_eq!(
        payload["summary"],
        "Logs of 5 failed jobs in run 77 of o/r. Showing the first 5 of 7; pass job_id for the others."
    );
}

#[tokio::test]
async fn workflow_logs_reads_one_job_when_asked() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), failed_job(2, Some("test"))] }),
        ),
        reply(200, json!("all fine")),
    ]);
    let payload = ok(
        &w,
        "workflow_logs",
        json!({ "repo": "o/r", "run_id": 77, "job_id": 1, "only_failed": true }),
    )
    .await;
    let calls = w.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].url, format!("{BASE}/actions/jobs/1/logs"));
    assert_eq!(
        payload["summary"],
        "Log of job 'build' (1) in run 77 of o/r."
    );
    assert_eq!(payload["jobs"][0]["log"], "all fine");
    assert_eq!(payload["job_count"], 1);
}

#[tokio::test]
async fn workflow_logs_refuses_a_job_not_in_the_run() {
    let w = with_token(vec![reply(
        200,
        json!({ "total_count": 1, "jobs": [job(json!({}))] }),
    )]);
    assert_matches(
        &refused(
            &w,
            "workflow_logs",
            json!({ "repo": "o/r", "run_id": 77, "job_id": 99 }),
        )
        .await,
        r"Job 99 is not among the 1 jobs listed for run 77 in o/r\. Read the run with workflow_status",
    );
}

#[tokio::test]
async fn workflow_logs_reads_every_job_when_only_failed_is_false() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), failed_job(2, Some("test"))] }),
        ),
        reply(200, json!("one")),
        reply(200, json!("two")),
    ]);
    let payload = ok(
        &w,
        "workflow_logs",
        json!({ "repo": "o/r", "run_id": 77, "only_failed": false }),
    )
    .await;
    assert_eq!(w.calls().len(), 3);
    assert_eq!(payload["summary"], "Logs of 2 jobs in run 77 of o/r.");
    let logs: Vec<Value> = payload["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["log"].clone())
        .collect();
    assert_eq!(logs, vec![json!("one"), json!("two")]);
}

#[tokio::test]
async fn workflow_logs_treats_a_missing_or_expired_log_as_a_note() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 3, "jobs": [failed_job(1, Some("a")), failed_job(2, Some("b")), failed_job(3, Some("c"))] }),
        ),
        reply(404, json!({ "message": "Not Found" })),
        reply(410, json!({ "message": "Gone" })),
        reply(200, json!("   \r\n")),
    ]);
    let payload = ok(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 77 })).await;
    let note = "GitHub has no log for this job: it may not have started, or the log has expired.";
    let jobs = payload["jobs"].as_array().unwrap();
    assert_eq!(jobs[0]["note"], note);
    assert_eq!(jobs[1]["note"], note);
    assert_eq!(jobs[2]["note"], "The log is empty.");
    assert!(
        jobs.iter()
            .all(|e| e.get("log").is_none() && e["truncated"] == false)
    );
    assert_eq!(payload["truncated"], false);
}

#[tokio::test]
async fn workflow_logs_says_why_there_is_nothing_to_read() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), job(json!({ "id": 2, "name": "test" }))] }),
        ),
        reply(
            200,
            json!({ "total_count": 2, "jobs": [job(json!({})), job(json!({ "id": 2, "name": "test", "status": "in_progress", "conclusion": null }))] }),
        ),
        reply(200, json!({ "total_count": 0, "jobs": [] })),
    ]);
    let args = json!({ "repo": "o/r", "run_id": 77 });
    let passed = ok(&w, "workflow_logs", args.clone()).await;
    assert_eq!(
        passed,
        json!({
            "summary": "Run 77 in o/r has no failed jobs. Pass only_failed false to read every job, or job_id for one.",
            "run_id": 77,
            "job_count": 0,
            "jobs": [],
            "truncated": false,
        })
    );
    let running = ok(&w, "workflow_logs", args.clone()).await;
    assert_eq!(
        running["summary"],
        "Run 77 in o/r has no failed jobs so far, and some jobs have not finished. Pass only_failed false to read every job, or job_id for one."
    );
    let queued = ok(&w, "workflow_logs", args).await;
    assert_eq!(queued["summary"], "Run 77 in o/r has no jobs yet.");
}

#[tokio::test]
async fn workflow_logs_does_not_swallow_a_refusal_not_about_the_log() {
    let w = with_token(vec![
        reply(
            200,
            json!({ "total_count": 1, "jobs": [failed_job(1, None)] }),
        ),
        reply(
            403,
            json!({ "message": "Resource not accessible by personal access token" }),
        ),
    ]);
    assert_matches(
        &refused(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 77 })).await,
        r"refused to read the log of job 1 of o/r \(403\)",
    );
}

#[tokio::test]
async fn workflow_logs_says_which_run_it_could_not_find_and_refuses_bad_arguments() {
    let w = with_token(vec![reply(404, json!({ "message": "Not Found" }))]);
    let cases = [
        (json!({ "repo": "o/r" }), r"`run_id` is required"),
        (
            json!({ "repo": "o/r", "run_id": 1.5 }),
            r"`run_id` must be a whole number",
        ),
        (
            json!({ "repo": "o/r", "run_id": 1, "job_id": 0 }),
            r"`job_id` must be between 1",
        ),
        (
            json!({ "repo": "o/r", "run_id": 1, "only_failed": "no" }),
            r"`only_failed` must be true or false",
        ),
    ];
    for (args, pattern) in cases {
        assert_matches(&refused(&w, "workflow_logs", args).await, pattern);
    }
    assert_eq!(w.calls().len(), 0);
    assert_matches(
        &refused(&w, "workflow_logs", json!({ "repo": "o/r", "run_id": 9 })).await,
        r"could not find workflow run 9 in o/r \(404\).*Find run ids with workflow_status",
    );
}
