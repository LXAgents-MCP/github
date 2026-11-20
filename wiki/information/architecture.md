# Architecture

A call goes from the client's arguments, through two checks, to one request to GitHub, and back as
text plus structured data. Everything that can go wrong on the way becomes a sentence the model can
act on.

## Where the code lives

`src/main.rs` is the binary's entry point and `src/lib.rs` the library root; both stay at the top. Everything else sits in a folder named for its role,
and a folder only imports from the ones below it in this table, so the transport never reaches into a tool
and a tool never reaches into the transport.

| Folder | Holds | Files |
|---|---|---|
| `src/routes/` | The ways in: the HTTP transport and its two routes, and the MCP server that registers the tools. | `http.rs`, `server.rs` |
| `src/controllers/` | One module per tool, and `mod.rs`, the registry that lists them. The product. | `repo_create.rs`, … |
| `src/services/` | Talking to GitHub: the context a call runs in, the one client that builds every URL, the two secrets, and the calls several tools share. | `context.rs` (`Ctx`: environment, network, sleep), `api.rs` (builds every URL and makes the call), `client.rs` (one request, capped), `failure.rs` (a status becomes a message), `auth.rs`, `contents.rs`, `releases.rs` |
| `src/models/` | What goes in and comes out: argument checks, the annotation sets and schema reader, and GitHub's objects as the tools report them. | `args.rs`, `address.rs` (repository, reference and path parts, validated and encoded), `paging.rs`, `schemas.rs`, `shapes.rs`, `tool.rs` (the contract a tool module meets), `body.rs` (a JSON object as GitHub returns it) |
| `src/utils/` | Helpers with no GitHub knowledge: rendering, text, `Refused`, the stderr logger. | `render.rs`, `text.rs`, `errors.rs`, `log.rs`, `redact.rs`, `http.rs` (JSON-RPC refusals, the capped body reader) |
| `src/config/` | Constants, and the version fixed from `Cargo.toml` at build time. | `config.rs`, `http_settings.rs` (`PORT` and `HOST`), `version.rs` |

Each file has one job; a file that grows a second one is split. A new module goes in the folder for what it does. A helper is never put in `src/controllers/`: anything there that
exports `def()` and `handle` and is registered in `mod.rs` is served as a tool.

## Source and build

The code is Rust, edition 2024, one Cargo package with a library (`src/lib.rs`) and one binary
(`src/main.rs`). `cargo build --release` compiles it to `target/release/lxagents-github`, which is what
runs — a client, the image — and it is never committed. The tests are one integration binary rooted at
`test/main.rs` that links the library, so `cargo test` builds what it needs itself.

Everything a tool reads from outside its arguments comes through a `Ctx` (`src/services/context.rs`): the
environment, read at call time, the network backend (`ReqwestBackend` in the server, `MockApi` in a test)
and sleep. A handler never reads `std::env` or opens a connection itself, which keeps the two secrets read
per call and lets tests run in parallel.

Types follow the layers. `src/models/tool.rs` is the contract every tool module meets (`ToolDef`,
`Outcome`, `Handler`, `ToolSpec`); an answer from GitHub is a JSON object read through `src/models/body.rs`,
field by field with a fallback, because a proxy or an outage can answer with anything.

## The tool surface is registered in one place

`src/controllers/mod.rs` declares every tool module and registers each with one line. A module exports
`def()` (its name, description, schemas and annotations) and `handle`; one that is not registered is never
callable. Nothing else names a tool: `src/routes/server.rs` serves whatever is registered, so the
protocol layer cannot grow a per-tool special case. A file's name is its tool's name in snake case
— `branch_list.rs` is `branch_list` — and a test holds files and tools together in both
directions. Two modules with one name stop the process at startup, not at the first call.

## A call, from argument to result

1. **The client calls a tool.** rmcp hands the arguments over as untyped JSON; the published input
   schema tells the client what to send.
2. **The handler checks the values.** `parse_repo`, `ref_name`, `file_path` (`src/models/address.rs`) and the
   helpers in `src/models/args.rs` refuse a wrong type, a bad repository, a name git would reject, a `..` segment or an out-of-range
   page — before the token is read and before anything is sent.
3. **`github()` builds the request** (`src/services/api.rs`). It reads the token from the `Ctx`'s environment, builds the URL under
   the API base, and pins `X-GitHub-Api-Version` and a `User-Agent`. It is the only code that builds
   a URL.
4. **`call_api` sends it** (`src/services/client.rs`) through the `Ctx`'s backend, `reqwest` with rustls. One request, with a 30 second timeout, and at most 20 MB of the answer
   is read.
5. **The status decides.** A 2xx answer, or a status the tool listed as an answer in its own right,
   goes back to the handler. Anything else becomes a `Refused` whose message names the status and
   the next step, with the token removed.
6. **The handler returns an `Outcome`**: a one-sentence `summary` and fields that satisfy the tool's
   output schema.
7. **`tool_result` renders it.** `src/utils/render.rs` turns the payload into the text the model reads —
   scalars as bullets, a list of rows as a heading and bullets, long or multi-line text as a fenced
   block — and the same fields go in `structuredContent`.

A returned `Err(Refused)` is an `isError` result carrying the reason and no structured content, because a client
validates structured content against the output schema and an empty object would be rejected for
the fields it lacks. A panic is caught, logged to stderr and returned as an internal error, and
the server carries on. Both are scrubbed of the token.

## One server instance per connection

A `GithubServer` is built fresh for each connection — one for stdio, one per HTTP request — from the
shared `Ctx`, and nothing hoists one to a static.

## Two transports

`src/main.rs` reads `MCP_TRANSPORT` and connects one of them. **stdio** is the default: the client
spawns the process and speaks to it over a pipe. **HTTP** (`MCP_TRANSPORT=http`) serves `POST /mcp` and
`GET /healthz` with axum and rmcp's stateless streamable-HTTP service; a stdio process never opens a
listener. The listener is bound in `src/routes/http.rs` and nowhere else.

Because stdout is the protocol channel on stdio, nothing in `src/` writes to it; diagnostics go to stderr
through `src/utils/log.rs`. The stdio process exits by itself when its client goes away, since stdin closes.
`SIGINT` and `SIGTERM` close either transport and exit cleanly; the second matters in a container,
where the server is the first process and would otherwise ignore it.

## The two secrets

`src/services/auth.rs` holds both and keeps them apart. **`GITHUB_API_KEY`** is the GitHub token, read from the
server's environment when a tool is called and used for every GitHub call on both transports; a request never
supplies it. **`MCP_AUTH_TOKEN`** guards the HTTP door: every request but `GET /healthz` must carry it as
`Authorization: Bearer <token>`, compared in constant time against the server's own, before the body is parsed
or a path matched. It is not forwarded, and HTTP that other machines can reach will not start without one of at
least 32 characters. A local install, listening on loopback with no token set, needs none: it answers only
requests whose `Host`, and `Origin` when sent, name this machine, which keeps a web page from reaching it through
DNS rebinding. stdio is a pipe the client spawned, so it has no access token and never reads the variable. The server stores nothing
between requests. See [Security model](../security/security-model.md).

## No local path is ever read from a caller

The server reads no local file: its version is fixed into the binary from `Cargo.toml` at build time. No argument
names a local path, and no tool reads or writes the local file system or runs a command. That is
`cli`'s job.

## Related pages

* [MCP surface](../reference/mcp-surface.md) — the conventions every tool shares.
* [Environment](../environments/env.md)
