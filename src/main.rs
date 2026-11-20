//! Server entry point.
//!
//! Gives an LLM access to GitHub over MCP, over stdio by default and over HTTP when `MCP_TRANSPORT=http`.
//! Two secrets, neither an argument:
//!
//! - `GITHUB_API_KEY` is the GitHub token, used for every call on both transports.
//! - `MCP_AUTH_TOKEN` is this server's own access token, which an HTTP client must present as
//!   `Authorization: Bearer <token>` (`src/routes/http.rs`). A server install - HTTP that other machines
//!   can reach - needs it; a local install, on loopback, does not. stdio never reads it.
//!
//! stdio: the client starts this process and speaks to it over a pipe; register it with the GitHub token
//! in its `env` block.
//!
//! ```json
//! {"mcpServers": {"lxagents-github": {
//!   "command": "<path to lxagents-github>",
//!   "env": {"GITHUB_API_KEY": "<token>"}}}}
//! ```
//!
//! The key this server uses can change and delete repositories. A pipe can only be reached by the client
//! that spawned it; the HTTP transport can be reached by anyone who can open its port. With an access token
//! it listens on port 3000 on all interfaces unless `PORT` and `HOST` say otherwise, and refuses any request
//! that does not carry the token. Without one it listens on 127.0.0.1 only, refuses to start on any address
//! that is not loopback, and refuses any request not addressed to loopback. The only other network traffic
//! is outbound, to api.github.com, and only when a tool is called.
//!
//! Nothing here may write to stdout: on stdio, stdout is the JSON-RPC channel.
use std::process::ExitCode;
use std::sync::Arc;

use lxagents_github::config::version::{SERVER_NAME, VERSION};
use lxagents_github::routes::http::{run_http, shutdown_signal};
use lxagents_github::routes::server::{GithubServer, install_panic_hook};
use lxagents_github::services::context::Ctx;
use lxagents_github::utils::log::log;
use rmcp::ServiceExt;

const USAGE: &str = "LXAgents GitHub MCP Server

Usage: lxagents-github [options]

Serves GitHub tools over stdio, or over HTTP when MCP_TRANSPORT=http. Neither secret is
an argument:
  GITHUB_API_KEY  the GitHub token; set it in the environment of this process, read when a tool
                  is called. Used on both transports.
  MCP_AUTH_TOKEN  http only: the server's access token, at least 32 characters, which every
                  request must carry as Authorization: Bearer <token>. Required for a server
                  install, where other machines can reach HTTP; optional for a local install,
                  on loopback. stdio does not read it.
  PORT (default 3000) and HOST say where HTTP listens. HOST defaults to 0.0.0.0, every
  interface, when MCP_AUTH_TOKEN is set, and to 127.0.0.1, this machine only, when it is not.

Options:
  -h, --help       Show this help
  -v, --version    Show the version
";

/// What the command line asked for.
enum Wanted {
    Serve,
    Help,
    Version,
}

/// Read the flags as `parseArgs` did with `strict: true` and no positionals: each argument must be one of
/// the two flags, in either spelling, and may repeat.
fn parse_flags(args: &[String]) -> Result<Wanted, String> {
    let (mut help, mut version) = (false, false);
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "-v" | "--version" => version = true,
            "-hv" | "-vh" => {
                help = true;
                version = true;
            }
            other if other.starts_with("--") && other.contains('=') => {
                let name = other.split('=').next().unwrap_or(other);
                return Err(format!("Option '{name}' does not take an argument"));
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("Unknown option '{other}'"));
            }
            other => {
                return Err(format!(
                    "Unexpected argument '{other}'. This command does not take positional arguments"
                ));
            }
        }
    }
    Ok(if help {
        Wanted::Help
    } else if version {
        Wanted::Version
    } else {
        Wanted::Serve
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_flags(&args) {
        Err(message) => {
            log(&format!("{message}\n\n{USAGE}"));
            return ExitCode::from(2);
        }
        Ok(Wanted::Help) => {
            log(USAGE);
            return ExitCode::SUCCESS;
        }
        Ok(Wanted::Version) => {
            log(&format!("{SERVER_NAME} {VERSION}"));
            return ExitCode::SUCCESS;
        }
        Ok(Wanted::Serve) => {}
    }

    install_panic_hook();
    let ctx = Arc::new(Ctx::production());

    // Which transport this process speaks, selected as `shared-instruction` selects it. stdio stays the
    // default, because running the binary with nothing set has to keep meaning what it meant.
    let raw = std::env::var("MCP_TRANSPORT")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let transport = if raw.is_empty() {
        "stdio".to_string()
    } else {
        raw
    };

    match transport.as_str() {
        "http" | "streamable-http" => ExitCode::from(run_http(ctx).await as u8),
        "stdio" => {
            // Said, and the signal handlers armed, before the client's handshake: a client that never
            // speaks, or goes away first, must not leave a process that ignores a signal or fails.
            let mut stop = Box::pin(shutdown_signal());
            let _ = futures::poll!(stop.as_mut());
            log(&format!("{SERVER_NAME} {VERSION} serving over stdio"));
            let serving = GithubServer::new(ctx).serve(rmcp::transport::stdio());
            let service = tokio::select! {
                served = serving => match served {
                    Ok(service) => service,
                    Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => {
                        return ExitCode::SUCCESS;
                    }
                    Err(error) => {
                        log(&format!(
                            "{SERVER_NAME} {VERSION} could not start over stdio: {error}"
                        ));
                        return ExitCode::from(1);
                    }
                },
                _ = stop.as_mut() => return ExitCode::SUCCESS,
            };
            // The client closing stdin ends the service; a signal ends the process.
            tokio::select! {
                _ = service.waiting() => {}
                _ = stop => {}
            }
            ExitCode::SUCCESS
        }
        other => {
            log(&format!(
                "MCP_TRANSPORT must be stdio or http, got '{other}'."
            ));
            ExitCode::from(2)
        }
    }
}
