# Architecture

A call goes from the client's arguments, through two checks, to one request to GitHub, and back as
text plus structured data. Everything that can go wrong on the way becomes a sentence the model can
act on.

## Where the code lives

`src/index.ts` is the entry point and stays at the top. Everything else sits in a folder named for its role,
and a folder only imports from the ones below it in this table, so the transport never reaches into a tool
and a tool never reaches into the transport.

| Folder | Holds | Files |
|---|---|---|
| `src/routes/` | The ways in: the HTTP transport and its two routes, and the MCP server that registers the tools. | `http.ts`, `server.ts` |
| `src/controllers/` | One module per tool, and `index.ts`, the registry that discovers them. The product. | `repo-create.ts`, … |
| `src/services/` | Talking to GitHub: the one client that builds every URL, the two secrets, and the calls several tools share. | `api.ts` (builds every URL and makes the call), `client.ts` (one request, capped), `failure.ts` (a status becomes a message), `auth.ts`, `contents.ts`, `releases.ts` |
| `src/models/` | What goes in and comes out: argument checks, the shared zod fragments, and GitHub's objects as the tools report them. | `args.ts`, `address.ts` (repository, reference and path parts, validated and encoded), `paging.ts`, `schemas.ts`, `shapes.ts`, `tool.ts` (the contract a tool module meets), `github-object.ts` (a JSON object as GitHub returns it) |
| `src/utils/` | Helpers with no GitHub knowledge: rendering, text, `Refused`, the stderr logger. | `render.ts`, `text.ts`, `errors.ts`, `log.ts`, `redact.ts`, `http.ts` (JSON-RPC refusals, the capped body reader) |
| `src/config/` | Constants, and the version read from `package.json`. | `config.ts`, `http-settings.ts` (`PORT` and `HOST`), `version.ts` |

Each file has one job; a file that grows a second one is split. A new module goes in the folder for what it does. A helper is never put in `src/controllers/`: anything there that
exports `TOOL` and `handle` is served as a tool.

## Source and build

The code is TypeScript under `strict`, and `npm run build` compiles `src/` to `dist/` with `tsc` (ES2022,
NodeNext). `dist/` is what runs — `npm start`, a client, the image — and it is never committed. Relative
imports name the `.js` file the compiler will write, so the same specifier resolves to `x.ts` in `src/` and
to `x.js` in `dist/`. The tests run the sources through `tsx` and need no build.

Types follow the layers. `src/models/tool.ts` is the contract every tool module meets (`ToolDefinition`,
`ToolResult`, `Payload`, `ToolSpec`); an answer from GitHub is a `GitHubObject` (`src/models/github-object.ts`),
read field by field with a fallback, because a proxy or an outage can answer with anything.

## The tool surface is discovered, not declared

`src/controllers/index.ts` imports every module in its own directory at startup: `.ts` files when it runs
from `src/`, `.js` files when it runs from `dist/`, never a declaration file or itself. A module that exports `TOOL`
(its name, description, schemas and annotations) and `handle(args)` is a tool; one that exports
neither is skipped. Nothing else names a tool: `src/routes/server.ts` registers whatever was found, so the
protocol layer cannot grow a per-tool special case. A file's name is its tool's name in kebab case
— `branch-list.ts` is `branch_list` — and a test holds files and tools together in both
directions. Two modules with one name stop the process at startup, not at the first call.

## A call, from argument to result

1. **The client calls a tool.** The MCP SDK checks the arguments against the tool's input schema,
   which catches a wrong type.
2. **The handler checks the values.** `parseRepo`, `refName`, `filePath` (`src/models/address.ts`) and the
   helpers in `src/models/args.ts` refuse a bad repository, a name git would reject, a `..` segment or an out-of-range
   page — before the token is read and before anything is sent.
3. **`github()` builds the request** (`src/services/api.ts`). It reads the token from the environment, builds the URL under
   the API base, and pins `X-GitHub-Api-Version` and a `User-Agent`. It is the only code that builds
   a URL.
4. **`callApi` sends it** (`src/services/client.ts`). One request, with a 30 second timeout, and at most 20 MB of the answer
   is read.
5. **The status decides.** A 2xx answer, or a status the tool listed as an answer in its own right,
   goes back to the handler. Anything else becomes a `Refused` whose message names the status and
   the next step, with the token removed.
6. **The handler returns a payload**: a one-sentence `summary` and fields that satisfy the tool's
   output schema.
7. **`toolResult` renders it.** `src/utils/render.ts` turns the payload into the text the model reads —
   scalars as bullets, a list of rows as a heading and bullets, long or multi-line text as a fenced
   block — and the same fields go in `structuredContent`.

A `Refused` is an `isError` result carrying the reason and no structured content, because a client
validates structured content against the output schema and an empty object would be rejected for
the fields it lacks. Any other exception is logged to stderr and returned as an internal error, and
the server carries on. Both are scrubbed of the token.

## One server instance per connection

`McpServer` holds per-connection state, so `createServer()` builds a fresh one each time and nothing
hoists one to module scope.

## Two transports

`src/index.ts` reads `MCP_TRANSPORT` and connects one of them. **stdio** is the default: the client
spawns the process and speaks to it over a pipe. **HTTP** (`MCP_TRANSPORT=http`, or `src/routes/http.ts`
directly) serves `POST /mcp` and `GET /healthz` with `node:http` and the SDK's stateless streamable-HTTP
transport; `src/index.ts` imports it only when asked, so a stdio process never loads it. There is no
other dependency for it, and `node:http` appears in `src/routes/http.ts` and nowhere else.

Because stdout is the protocol channel on stdio, nothing in `src/` writes to it; diagnostics go to stderr
through `src/utils/log.ts`. The stdio process exits by itself when its client goes away, since stdin closes.
`SIGINT` and `SIGTERM` close either transport and exit cleanly; the second matters in a container,
where the server is the first process and would otherwise ignore it.

## The two secrets

`src/services/auth.ts` holds both and keeps them apart. **`GITHUB_API_KEY`** is the GitHub token, read from the
server's environment when a tool is called and used for every GitHub call on both transports; a request never
supplies it. **`MCP_AUTH_TOKEN`** guards the HTTP door: every request but `GET /healthz` must carry it as
`Authorization: Bearer <token>`, compared in constant time against the server's own, before the body is parsed
or a path matched. It is not forwarded, and HTTP that other machines can reach will not start without one of at
least 32 characters. A local install, listening on loopback with no token set, needs none: it answers only
requests whose `Host`, and `Origin` when sent, name this machine, which keeps a web page from reaching it through
DNS rebinding. stdio is a pipe the client spawned, so it has no access token and never reads the variable. The server stores nothing
between requests. See [Security model](../security/security-model.md).

## No local path is ever read from a caller

The server reads one local file, its own `package.json`, once, to learn its version. No argument
names a local path, and no tool reads or writes the local file system or runs a command. That is
`cli`'s job.

## Related pages

* [MCP surface](../reference/mcp-surface.md) — the conventions every tool shares.
* [Environment](../environments/env.md)
