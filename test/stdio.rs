//! Ported from test/stdio.test.ts: the real binary, spoken to over a real pipe.
use std::process::{Output, Stdio};
use std::time::Duration;

use lxagents_github::config::version::{SERVER_NAME, VERSION};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::helpers::assert_matches;
use crate::server::Rpc;
use crate::surface::tool_names;

const BIN: &str = env!("CARGO_BIN_EXE_lxagents-github");

/// The binary with neither secret in its environment.
fn server() -> Command {
    let mut command = Command::new(BIN);
    command
        .env_remove("GITHUB_API_KEY")
        .env_remove("MCP_AUTH_TOKEN")
        .env_remove("MCP_TRANSPORT")
        .kill_on_drop(true);
    command
}

async fn run(args: &[&str]) -> Output {
    server()
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .unwrap()
}

#[tokio::test]
async fn over_stdio_the_server_lists_exactly_its_surface_and_answers_a_call() {
    let mut child = server()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut client = Rpc::connect(child.stdout.take().unwrap(), child.stdin.take().unwrap()).await;
    let tools = client.tools().await;
    let mut names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, tool_names());

    let result = client
        .call("repo_create", serde_json::json!({"name": "demo"}))
        .await;
    assert_eq!(result["isError"], true);
    assert_eq!(result["content"][0]["type"], "text");
    assert_matches(
        result["content"][0]["text"].as_str().unwrap(),
        "GITHUB_API_KEY is not set",
    );
    let _ = child.kill().await;
}

#[tokio::test]
async fn a_bad_command_line_exits_2_with_the_reason_on_stderr_and_nothing_on_stdout() {
    for args in [["--bogus"], ["stray"], ["--help=1"]] {
        let out = run(&args).await;
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
        assert_matches(
            &String::from_utf8_lossy(&out.stderr),
            "Usage: lxagents-github",
        );
    }
}

#[tokio::test]
async fn help_prints_the_usage_to_stderr_leaves_stdout_empty_and_exits_0() {
    let out = run(&["--help"]).await;
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    assert_matches(&String::from_utf8_lossy(&out.stderr), "GITHUB_API_KEY");
}

#[tokio::test]
async fn version_prints_the_name_and_version_to_stderr_leaves_stdout_empty_and_exits_0() {
    let out = run(&["--version"]).await;
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim(),
        format!("{SERVER_NAME} {VERSION}")
    );
}

/// The exit code, or `None` (after killing it) if it is still running after five seconds.
async fn exit_within(child: &mut Child) -> Option<i32> {
    match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
        Ok(status) => status.unwrap().code(),
        Err(_) => {
            let _ = child.kill().await;
            None
        }
    }
}

async fn signal_exits_0(signal: &str) {
    let mut child = server()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The handlers are installed before this line is written.
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    while let Some(line) = lines.next_line().await.unwrap() {
        if line.contains("serving over stdio") {
            break;
        }
    }
    let pid = child.id().unwrap().to_string();
    let killed = std::process::Command::new("kill")
        .args([&format!("-{signal}"), &pid])
        .status()
        .unwrap();
    assert!(killed.success());
    assert_eq!(exit_within(&mut child).await, Some(0), "{signal}");
}

#[tokio::test]
async fn sigint_closes_the_server_and_exits_0() {
    signal_exits_0("INT").await;
}

#[tokio::test]
async fn sigterm_closes_the_server_and_exits_0() {
    signal_exits_0("TERM").await;
}

#[tokio::test]
async fn the_process_exits_by_itself_when_its_client_goes_away() {
    let mut child = server()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    assert_eq!(exit_within(&mut child).await, Some(0));
}
