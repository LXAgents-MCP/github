---
name: github-repository-rules
description: Rules specific to LXAgents-MCP/github — its two roles, the stack and commands, the code conventions, and what must not be introduced.
---

# GitHub Repository Rules

Rules that are true of **this repository only**. Anything true of more than one repository is
proposed to the owner for the shared set, not written here.

This file is a hub. It links out rather than restating, and it never repeats a rule the shared
set already carries.

## This repository has two roles

| Role | Meaning |
|---|---|
| **Consumer** | It follows the shared instruction set, read through the connector, plus the local additions in `.agents/`. |
| **Producer** | It serves the GitHub tool surface: one module per tool in `src/controllers/`. |

Keep them apart. `.agents/` is this repository's own instruction set; `src/controllers/` is the product.
Nothing from the shared set is copied here.

## What lives where

| Path | Holds |
|---|---|
| `src/` | The server: `main.rs` (the binary), `lib.rs` and one folder per role — `routes/`, `controllers/`, `services/`, `models/`, `utils/`, `config/` (see [Where the code lives](../../wiki/information/architecture.md#where-the-code-lives)). `src/controllers/` is the tool surface; everything else in `src/` is shared by it. |
| `test/` | The server's tests. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. |
| `wiki/` | Human documentation, and the release logs under `wiki/logs/`. |
| `Dockerfile`, `.dockerignore` | The image: stdio by default, HTTP with `MCP_TRANSPORT=http`. See [`wiki/environments/docker.md`](../../wiki/environments/docker.md). |

Full orientation: [`../wiki/context/repository-map.md`](../wiki/context/repository-map.md).

## Stack and commands

Rust, edition 2024, `rust-version = "1.88"`: one Cargo package with a library and the `lxagents-github`
binary. `cargo build --release` produces `target/release/lxagents-github`; nothing compiled is committed.
Do not introduce another language, a second crate, or a build script without agreement.

| Task | Command |
|---|---|
| Build | `cargo build --release` |
| Test | `cargo test` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Format | `cargo fmt` (check: `cargo fmt --check`) |
| Run (stdio) | `target/release/lxagents-github` |
| Run (HTTP) | `MCP_TRANSPORT=http target/release/lxagents-github` |
| Inspect the MCP surface | `npx @modelcontextprotocol/inspector target/release/lxagents-github` |
| Build the image | `docker build -t "lxagents-github:$VERSION" .` — it has never been built here |

**A client runs the built binary, not the sources.** `cargo test` compiles what it needs, but a client
configured with `target/release/lxagents-github` keeps serving the old build until `cargo build --release`
runs again.

## Code conventions the codebase already follows

* **Nothing writes to stdout.** On stdio, stdout *is* the JSON-RPC channel. Diagnostics go through
  `src/utils/log.rs`, which writes to stderr, on the HTTP transport too. There is no `print!` or `println!`
  anywhere in `src/`.
* **One `GithubServer` per request.** The HTTP transport's factory builds a fresh instance from the shared
  `Ctx`; never hoist one to a static.
* **Every URL is built in `src/services/api.rs`.** A repository, a reference and a file path are validated
  and percent-encoded in `src/models/address.rs`, and a tool never concatenates its own.
* **Arguments are checked before the token is read**, so a bad call costs no request and the
  refusal names the argument.
* **The GitHub key is read per call**, through `src/services/auth.rs` and nowhere else, from `GITHUB_API_KEY`, on
  both transports. It is never an argument, and is scrubbed from every message — refusals and crashes
  alike. **The access token is a different secret**: `MCP_AUTH_TOKEN` guards the HTTP door of a server
  install, is compared and dropped, and is never used against GitHub or read over stdio. A local install,
  on loopback with no token, is guarded by the `Host` and `Origin` checks in `src/routes/http.rs` instead.
* **Everything from outside comes through `Ctx`** (`src/services/context.rs`): the environment, the network
  backend and sleep. A handler never reads `std::env` or opens a connection itself.
* **A failure is a returned `Err(Refused)`** that says what to do next. A tool never returns an error
  payload; anything else is a bug, a panic, which the dispatcher reports as an internal error rather than
  as its explanation.
* **Comments explain why, not what.**

## Testing

Every behavioural change ships with a test in `test/`, one integration binary rooted at `test/main.rs`
and run by `cargo test`. The suite must pass before any commit, and **no test reaches GitHub or any host
beyond this machine**: each test hands the tools a `Ctx` whose backend is `MockApi` in `test/helpers.rs`,
which fails the test on any request it was not told to expect. The one exception is the redirect test in
`test/api.rs`, which uses the real `reqwest` backend against two servers on loopback, to show that the token
is not sent to a second host.

Each tool is tested for: a success that asserts the exact request — method, URL, query, body — and
checks the payload against the tool's own output schema with `expect_output`; at least one failure
status; and argument validation that refuses before any request. `test/surface.rs` is the one
place the tool list is pinned. Prefer a test that pins an invariant over one that pins a string.

The suite lives in `test/`, not Cargo's default `tests/`: `Cargo.toml` names it (`[[test]] suite`) and
sets `autotests = false`, so a file there is compiled only when `test/main.rs` declares it as a module.

## What must not be introduced

* Any compiled output committed to the repository, or a build script (`build.rs`).
* A dependency added to serve one call site. The runtime dependencies are the set in `Cargo.toml`
  (rmcp, axum, reqwest with rustls, tokio and their helpers), recorded in
  [`../memory/decisions/rust-runtime-dependencies.md`](../memory/decisions/rust-runtime-dependencies.md); another needs its own record under `.agents/memory/decisions/` before it lands, not after.
* A transport other than stdio and the HTTP one in `src/routes/http.rs`, a listener anywhere else, a route
  beyond `POST /mcp` and `GET /healthz`, a way for an HTTP request to get through without the access
  token when one is set, HTTP starting without one on an address that is not loopback, a local install
  without the `Host` and `Origin` checks, a client's bearer used as the GitHub key (or the other way round),
  or a store of tokens between requests. See
  [`../memory/decisions/two-token-auth.md`](../memory/decisions/two-token-auth.md). The
  image follows: no `EXPOSE`, no `ENTRYPOINT`, no `ENV` line holding a secret, no compose file.
* An environment variable, flag or configuration file without the owner's say-so. The server reads
  `GITHUB_API_KEY` and `MCP_AUTH_TOKEN` (the two secrets) and `MCP_TRANSPORT`, `PORT` and `HOST`, and takes
  `--help` and `--version`; another of either is a change to a public surface, not an implementation
  detail.
* A tool argument that carries a credential, or any way for a caller to choose the host. The API
  base is a constant in `src/config/config.rs`, which is also why GitHub Enterprise is not supported.
* A URL built anywhere but `src/services/api.rs`.
* A third documentation tree. `wiki/` and `.agents/wiki/` are the only two.
* `content/`, or any copy of the shared set. This repository reads it through the connector.

## Deployment

Not published: `Cargo.toml` sets `publish = false`, and the client runs the built binary by path
(`target/release/lxagents-github`) with the key in the `env` block of its own configuration, or runs it as an
HTTP service the client reaches with an access token. Making it
installable from crates.io is a one-line change the owner approves, and not one to make in passing. A
`Dockerfile` builds an image that speaks stdio for a client that runs `docker run -i`, or HTTP when
told to; publishing it to a registry is the owner's decision as well.
