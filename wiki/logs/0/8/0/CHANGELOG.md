# 0.8.0

**Released:** 2026-11-20

**The server is Rust, and what runs is one binary, `lxagents-github`.** The crate is built on `rmcp` 3.10,
the official Rust MCP SDK, with `axum` 0.8 for the HTTP transport. The 45 tools, their arguments, results
and messages, the tokens, the variables and the routes are as they were: `tools/list` matches 0.7.0's
field for field, except the `execution` key the TypeScript SDK added, which is gone. **Breaking for the
entry point:** `dist/index.js` no longer exists; a client runs `target/release/lxagents-github`, which a
checkout has only after `cargo build --release`.

**Clients must:** install a Rust toolchain (1.88 or newer), run `cargo build --release` in the checkout,
then change the server's command from `node .../dist/index.js` to `.../target/release/lxagents-github`
(set `MCP_TRANSPORT=http` for HTTP). Build again after every pull. Rebuild the image. Restart the client.

## Added

* `Cargo.toml` and `Cargo.lock`: a library, `lxagents_github`, and the binary that serves it. Runtime
  dependencies and why each is there: `.agents/memory/decisions/rust-runtime-dependencies.md`.
* `test/fixtures/tools-list.json`, 0.7.0's `tools/list`, and a test that holds the Rust surface to it.

## Changed

* Every file in `src/` and `test/` is Rust; tools are `src/controllers/<name>.rs`, registered in
  `src/controllers/mod.rs`.
* GitHub is reached through `reqwest` with `rustls`. Tools receive their environment, network and log
  through an injected context, so the 315 tests run in parallel against a mock backend.
* `cargo test` runs the suite, `cargo clippy --all-targets -- -D warnings` and `cargo fmt` lint it.
* `--help` says `Usage: lxagents-github [options]`.
* The image is built from `rust:1-alpine` into a bare `alpine:3` and runs the binary as `nobody`.
* A string limit counts characters, not UTF-16 code units, except a git ref name's 255.
* A crash in a tool reads `Error: <message>`, and an argument of the wrong type gets the tool's own
  refusal rather than the SDK's validation message.

## Removed

* `package.json`, the lockfile, `tsconfig*.json`, `dist/`, the MCP TypeScript SDK and zod.

## Fixed

* `issue_assign`'s summary ended in a space after a removal GitHub did not apply.

## Verified

`cargo build`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass; `cargo test`:
315 passing, none failing, every 0.7.0 case ported. The binary answered `--version`, `--help`,
`tools/list` and tool calls over stdio and over HTTP in the suite. The image was built up to the
dependency download, which the work environment's network refused, so it has still never been built
whole. Still nothing run against live GitHub.
