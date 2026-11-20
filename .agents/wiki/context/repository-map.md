---
name: agent-wiki-context-repository-map
description: Orientation before touching this repository — what lives where, the commands, the entry point, and the gotchas that actually bite.
---

# Repository Map

Read this before changing anything here. Underlying facts live in `wiki/`; this page is the
agent-facing orientation and links out rather than restating.

## What this repository is

`LXAgents-MCP/github` — an MCP server named `lxagents-github` that gives an LLM access to GitHub
through the REST API, over stdio or HTTP. Rust (edition 2024), compiled to one binary, `target/release/lxagents-github`, by `cargo build --release`. It is a **consumer**
of the shared instruction set and the **producer** of the tool surface in `src/controllers/`; see
[`../../rules/repository.md`](../../rules/repository.md).

## What lives where

`src/` is `main.rs` (the binary) and `lib.rs` (the library the tests link) plus six folders by role, each with its `mod.rs` — `routes/`, `controllers/` (the tools), `services/`, `models/`,
`utils/` and `config/`. Which module goes where, and why, is in
[`wiki/information/architecture.md`](../../../wiki/information/architecture.md#where-the-code-lives).

| Path | Contents | Touch it when |
|---|---|---|
| `src/main.rs` | The one entry point: `--help`, `--version`, `MCP_TRANSPORT`, the stdio transport, `SIGINT` and `SIGTERM`, the panic hook. | The process contract. |
| `src/lib.rs` | The library root: declares the six folders, which import downward only. | A new folder. |
| `src/routes/http.rs` | The HTTP transport (axum and rmcp's streamable HTTP service) and the only listener: `POST /mcp`, `GET /healthz`, a fresh stateless server per request; `PORT` and `HOST` are read in `src/config/http_settings.rs`. What `MCP_TRANSPORT=http` runs. | Anything about serving over HTTP. |
| `src/services/auth.rs` | The two secrets: the access token for HTTP (`MCP_AUTH_TOKEN`: the 32-character floor, the bearer parser, the constant-time comparison) and the GitHub key (`GITHUB_API_KEY`). | Anything about either secret. |
| `src/routes/server.rs` | `GithubServer`, the rmcp handler: lists and dispatches every registered tool, catches a panic as an internal error, shapes results, and carries the `instructions` text every client reads at `initialize`. | The protocol surface. |
| `src/services/api.rs` | `github()`: the one place a GitHub URL is built and a call is made. | How a call is addressed. |
| `src/services/client.rs`, `src/services/failure.rs` | `call_api`, one request with a timeout and a size cap, and `ReqwestBackend`, the real network; and the wording of every failure. |
| `src/services/context.rs` | `Ctx`: the environment, the network backend and sleep, handed to every tool, so a test swaps in a map and `MockApi`. | Anything a tool reads from outside its arguments. | How a call is made, or fails. |
| `src/models/address.rs`, `src/models/paging.rs` | `parse_repo`, `ref_name`, `ref_path`, `file_path`, `workflow_name`: path parts validated and encoded; `page_query` and `paging`. | A new kind of path part, or paging. |
| `src/models/args.rs` | Argument checks: text, flags, whole numbers, choices, lists. | A new kind of argument. |
| `src/utils/render.rs` | The one renderer from a payload to the text the model reads. | How a result reads. |
| `src/models/schemas.rs` | The four annotation sets (`READ`, `WRITE`, `UPDATE`, `DESTROY`) and `schema`, which reads a tool's hand-written JSON schema. | A fragment or hint several tools share. |
| `src/utils/text.rs` | `short`, `first_line`, `head`, `tail`, `cut_text`, `decode_text`, `looks_binary`, `clean_log`; lengths in Unicode scalar values, cut on `char` boundaries. | Cutting, decoding or cleaning text. |
| `src/models/shapes.rs`, `src/models/body.rs` | GitHub's objects as the tools report them — a repository, a release, an issue, a job — each a schema and the function that fills it, and `expect_body`, which checks an answer is the object expected; `body.rs` reads fields out of a GitHub JSON object. | A second tool returns an object another already does. |
| `src/services/contents.rs`, `src/services/releases.rs` | What several tools share: a file's path, encoding and blob sha; finding a release by its tag, drafts included. | Those two subjects. |
| `src/config/config.rs` | Every constant: the variable name, API base and version pin, timeout, limits, the HTTP defaults. | Changing a limit. |
| `src/utils/errors.rs`, `src/utils/log.rs`, `src/config/version.rs` | `Refused`; the stderr logger; `VERSION` (from `Cargo.toml`) and `SERVER_NAME`. | Rarely. |
| `src/controllers/` | `mod.rs`, the registry, and one module per tool. | Adding or changing a tool — **a release**. |
| `Dockerfile`, `.dockerignore` | The image (stdio by default, HTTP when told to): a `rust:1-alpine` build stage that runs `cargo build --release --locked`, then an `alpine:3` runtime stage (its CA bundle included), the static binary at `/usr/local/bin/lxagents-github`, `USER nobody` and `CMD ["lxagents-github"]`. It has never been built. | The image changes — and say it is unbuilt if it still is. |
| `Cargo.toml`, `Cargo.lock` | The package: one library, the `lxagents-github` binary, the `suite` test binary rooted at `test/main.rs` (`autotests = false`), the dependencies and the release profile. | A dependency or a build setting changes. |
| `target/` | Build output, ignored by git. Not a source: edit `src/` and build. | Never by hand. |
| `test/` | One integration binary: `main.rs` declares one module per subject, plus `helpers.rs` and `surface.rs`, which every other module uses, and `fixtures/tools-list.json`, the pinned published schemas. | Always. Every behavioural change ships with a test. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. | You are changing something true only here. |
| `wiki/` | Human documentation, and `wiki/logs/` for release history. | A person needs to read it. |

There is no `src/transport/` and no `content/`; HTTP is `src/routes/http.rs` and nothing else. If a file you are about to
edit is under either, it does not exist here.

## Entry point

There is **one**: `src/main.rs`, compiled to `target/release/lxagents-github`, which is what an MCP client spawns. It reads `MCP_TRANSPORT`: stdio connects a
stdio transport and exits by itself when its client goes away, because stdin closes; `http` hands over to
`src/routes/http.rs`. Either handles `SIGINT` and `SIGTERM`, and nothing writes to stdout. Behind it,
`GithubServer` in `src/routes/server.rs` serves whatever `src/controllers/mod.rs` registers.

## Commands

```bash
cargo build --release                       # src/ to target/release/lxagents-github; again after every change to src/
cargo test                                  # the one test binary; builds what it needs
cargo clippy --all-targets -- -D warnings   # lints src/ and test/, warnings are errors
cargo fmt                                   # format; --check to verify only
target/release/lxagents-github              # stdio; GITHUB_API_KEY comes from the environment
MCP_TRANSPORT=http target/release/lxagents-github   # HTTP on :3000: with MCP_AUTH_TOKEN (32+ chars) on all interfaces, without it on 127.0.0.1 only
GITHUB_API_KEY=<token> npx @modelcontextprotocol/inspector target/release/lxagents-github   # the MCP Inspector, to drive it by hand
```

## Gotchas that actually bite

* **Nothing writes to stdout.** stdout is the JSON-RPC channel. A stray write corrupts the stream
  and the client reports a JSON parse error that points at the payload, not at the logging.
* **Starting without the GitHub key is fine; HTTP without the access token is fine only on loopback.** The key is read when
  a tool runs; without it the tool refuses and names `GITHUB_API_KEY`, which is not a broken server. HTTP
  with no `MCP_AUTH_TOKEN` on an address that is not loopback, or a token under 32 characters anywhere, exits 1 on
  purpose. With neither `MCP_AUTH_TOKEN` nor `HOST` set it is a local install on `127.0.0.1`, and says so.
* **A server registered mid-session is absent until the session restarts.** The client loads its
  connectors at start.
* **The tool list is pinned.** Add a tool without its line in `test/surface.rs` and three tests
  fail. That is the design, not friction.
* **`MockApi` fails on any request it was not told about.** If a tool makes one more call than the
  test queued, the failure reads `unexpected request #n`, and it is correct.
* **`target/release/` can be stale.** Every client runs the built binary, not `src/`. After a change,
  `cargo test` passes on the sources while a client still runs the old build until `cargo build --release`
  runs again.
* **A test file not declared in `test/main.rs` is never compiled.** `autotests = false` and the suite
  lives in `test/`, not Cargo's `tests/`, so a new file needs its `mod` line or it silently runs nothing.
* **A tool not listed in `src/controllers/mod.rs` does not exist.** It needs both its `pub mod` line and its
  `spec!` line; the registry test catches a file without them.
* **GitHub answers 404 for a private repository the token cannot see**, so a 404 can mean
  "not visible", not "does not exist". The refusal says so.
* **A tool file's name is its tool's name.** Renaming the file is renaming the tool, which is a
  breaking change for any client that calls it.

## Generated and vendored paths

`target/` only. Nothing is generated into the tracked tree.

## Where the shared set resolves from

The connector `lxagents-shared-instruction`, never a file in this repository.

## Further reading

* [`../../../wiki/information/architecture.md`](../../../wiki/information/architecture.md) — how the server is built and why.
* [`../../../wiki/reference/mcp-surface.md`](../../../wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
* [`../../../wiki/environments/env.md`](../../../wiki/environments/env.md) — the one variable, and the token's required permissions.
* [`../security/security-boundaries.md`](../security/security-boundaries.md) — the security procedure.
