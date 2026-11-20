# Local Setup

An MCP server over stdio, or over HTTP behind an access token. The only flags are `--help` and `--version`;
the transport is chosen by `MCP_TRANSPORT`.

## Requirements

Rust 1.88 or newer (edition 2024), with Cargo. The server is one Cargo package, compiled to a single
binary, `target/release/lxagents-github`.

```bash
cargo build --release
cargo test
```

`cargo build --release` compiles `src/` to `target/release/lxagents-github`, the only thing the commands
below and a client run. It is not committed, so build after cloning and after every change to `src/`; a
stale binary serves the old code. Before a commit, also run
`cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` (`cargo fmt` to fix).

The suite hands every tool a mock network, `MockApi`, through its `Ctx`, and reaches no host beyond loopback. It drives a real MCP client against the
server on an in-memory transport, spawns the real process to check what only a pipe shows (a stray
write to stdout, a process that outlives its client), and tests each tool for the exact request it
sends, the payload it returns, and the refusals it makes. The tests are one integration binary rooted
at `test/main.rs`, which `cargo test` builds along with the server binary it spawns, so they never depend
on a stale build. The two support modules, `test/helpers.rs` and `test/surface.rs`, define no tests.

## Run

A client normally starts the server, so "running" it means pointing the client at it — see
[Connect a client](../guides/connect-a-client.md). To run it by hand:

```bash
GITHUB_API_KEY=<token> target/release/lxagents-github
```

It then waits on stdin for a client. Press Ctrl-C to stop it, or close its stdin. Without the
variable it still starts; a tool then refuses and names the variable.

To serve HTTP on this machine only, a local install, give it the GitHub key and nothing else. It listens on
`127.0.0.1` and asks for no access token:

```bash
GITHUB_API_KEY=<token> MCP_TRANSPORT=http target/release/lxagents-github   # http://127.0.0.1:3000/mcp, this machine only
```

To serve HTTP to other machines, a server install, also give it an access token for clients (at least 32
characters; on any address but loopback it will not start without one):

```bash
GITHUB_API_KEY=<token> MCP_AUTH_TOKEN=$(openssl rand -hex 32) MCP_TRANSPORT=http \
  target/release/lxagents-github   # port 3000, all interfaces
```

See [Set up HTTP with two tokens](../guides/setup-http-token.md). The two flags:

```bash
target/release/lxagents-github --help       # usage, on stderr
target/release/lxagents-github --version    # name and version, on stderr
```

## Inspect it

```bash
GITHUB_API_KEY=<token> npx @modelcontextprotocol/inspector target/release/lxagents-github
```

opens the MCP Inspector (it needs Node.js) against the stdio server, so you can list the tools and call one by hand.
This is the quickest way to see the surface a fresh process would build.

## stdout belongs to the protocol

On stdio, stdout is the JSON-RPC channel. One stray write corrupts the stream, and the client
reports a JSON parse error that points at the payload rather than at the logging. Nothing in `src/`
writes to stdout; diagnostics go to stderr, which is why `--help` and `--version` print there.

## Tool changes require a restart

The registry is read once, at startup. Editing `src/controllers/` has no effect on a running process:
rebuild with `cargo build --release`, then restart the client or the server.

## Related pages

* [Environment](env.md) — the variables.
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
* [Docker](docker.md) — the same server in a container.
* [Architecture](../information/architecture.md)
