//! Ported from test/auth.test.ts.
use lxagents_github::config::config::{AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV};
use lxagents_github::services::auth::{
    auth_token_problem, bearer_credential, configured_api_key, configured_auth_token,
    credential_matches, digest, is_loopback_host, require_token, token_to_scrub,
};

use crate::helpers::{TOKEN, assert_matches, with_token, without_token};

fn access() -> String {
    "0123456789abcdef".repeat(4)
}

#[test]
fn the_variables_are_the_ones_the_owner_named() {
    assert_eq!(TOKEN_ENV, "GITHUB_API_KEY");
    assert_eq!(AUTH_ENV, "MCP_AUTH_TOKEN");
    assert_eq!(MIN_AUTH_TOKEN_LENGTH, 32);
}

#[test]
fn bearer_credential_reads_the_credential_of_a_bearer_header_and_nothing_else() {
    assert_eq!(
        bearer_credential(Some("Bearer abc")).as_deref(),
        Some("abc")
    );
    assert_eq!(
        bearer_credential(Some("bearer    abc")).as_deref(),
        Some("abc")
    );
    assert_eq!(
        bearer_credential(Some("BEARER abc")).as_deref(),
        Some("abc")
    );
    assert_eq!(bearer_credential(None), None);
    for header in [
        "",
        "Bearer",
        "Bearer ",
        "Basic abc",
        "Bearer a b",
        "Token abc",
        "Bearerabc",
    ] {
        assert_eq!(bearer_credential(Some(header)), None, "{header:?}");
    }
}

#[test]
fn the_access_token_is_read_at_call_time_and_trimmed() {
    let world = without_token(vec![]);
    assert_eq!(configured_auth_token(world.env.as_ref()), "");
    world.env.set(AUTH_ENV, &format!("  {}\n", access()));
    assert_eq!(configured_auth_token(world.env.as_ref()), access());
}

#[test]
fn auth_token_problem_refuses_a_missing_or_short_token_and_never_names_the_value() {
    assert_eq!(
        auth_token_problem("").as_deref(),
        Some("MCP_AUTH_TOKEN is not set")
    );
    assert_eq!(
        auth_token_problem(&"a".repeat(31)).as_deref(),
        Some("MCP_AUTH_TOKEN is 31 characters, and it must be at least 32")
    );
    assert_eq!(auth_token_problem(&"a".repeat(32)), None);
    assert_eq!(auth_token_problem(&access()), None);
    assert!(
        !auth_token_problem("short-secret")
            .unwrap()
            .contains("short-secret")
    );
}

#[test]
fn auth_token_problem_reads_the_environment_when_given_no_token() {
    let world = without_token(vec![]);
    let env = world.env.as_ref();
    assert_eq!(
        auth_token_problem(&configured_auth_token(env)).as_deref(),
        Some("MCP_AUTH_TOKEN is not set")
    );
    world.env.set(AUTH_ENV, &access());
    assert_eq!(auth_token_problem(&configured_auth_token(env)), None);
}

#[test]
fn credential_matches_accepts_the_token_and_nothing_that_merely_resembles_it() {
    let a = access();
    let expected = digest(&a);
    assert!(credential_matches(&a, &expected));
    for wrong in [
        a[..a.len() - 1].to_string(),
        a[1..].to_string(),
        format!("{a}0"),
        a.to_uppercase(),
        String::new(),
        "x".into(),
        "a".repeat(5000),
    ] {
        assert!(
            !credential_matches(&wrong, &expected),
            "{}",
            &wrong[..wrong.len().min(20)]
        );
    }
}

#[test]
fn the_github_key_is_the_one_in_github_api_key_read_per_call_and_trimmed() {
    let world = without_token(vec![]);
    world.env.set(TOKEN_ENV, &format!("  {TOKEN}  "));
    assert_eq!(configured_api_key(world.env.as_ref()), TOKEN);
    assert_eq!(require_token(&world.ctx).unwrap(), TOKEN);
    assert_eq!(token_to_scrub(&world.ctx), TOKEN);

    world.env.set(TOKEN_ENV, "   ");
    assert_eq!(configured_api_key(world.env.as_ref()), "");
    assert_eq!(token_to_scrub(&world.ctx), "");
    assert_matches(
        require_token(&world.ctx).unwrap_err().message(),
        r"^GITHUB_API_KEY is not set\. .*`env` block",
    );
}

#[test]
fn mcp_auth_token_is_not_a_github_key() {
    let world = without_token(vec![]);
    world.env.set(AUTH_ENV, &access());
    assert_eq!(configured_api_key(world.env.as_ref()), "");
    assert_matches(
        require_token(&world.ctx).unwrap_err().message(),
        r"^GITHUB_API_KEY is not set\.",
    );
}

#[test]
fn github_api_key_is_not_an_access_token() {
    let world = with_token(vec![]);
    assert_eq!(configured_auth_token(world.env.as_ref()), "");
    assert_eq!(
        auth_token_problem(&configured_auth_token(world.env.as_ref())).as_deref(),
        Some("MCP_AUTH_TOKEN is not set")
    );
    assert!(!credential_matches(TOKEN, &digest(&access())));
}

#[test]
fn is_loopback_host_knows_this_machine_only_by_its_literal_names() {
    for name in [
        "localhost",
        "LOCALHOST",
        "localhost.",
        " localhost ",
        "127.0.0.1",
        "127.1.2.3",
        "127.255.255.255",
        "::1",
        "[::1]",
    ] {
        assert!(is_loopback_host(name), "{name}");
    }
    for name in [
        "0.0.0.0",
        "::",
        "10.0.0.1",
        "192.168.1.10",
        "128.0.0.1",
        "127.0.0.1.nip.io",
        "localhost.example.com",
        "evil-localhost",
        "127.0.0",
        "127.0.0.256",
        "[::1",
        "",
    ] {
        assert!(!is_loopback_host(name), "{name}");
    }
}
