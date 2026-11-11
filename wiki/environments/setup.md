# Local Setup

An MCP server over stdio, or over HTTP behind an access token. The only flags are `--help` and `--version`;
the transport is chosen by `MCP_TRANSPORT`.

## Requirements

Node.js 20 or newer. The server is TypeScript (ESM), compiled to `dist/` with `tsc`.

```bash
npm install
npm run build
npm test
```

`npm install` first, once per checkout: a fresh clone has no `node_modules`, and the suite then
fails with `ERR_MODULE_NOT_FOUND`, which reads as broken code rather than an uninstalled tree.

`npm run build` compiles `src/` to `dist/`, the only thing `npm start`, `npm run start:http`,
`npm run inspect` and a client run. It is not committed, so build after cloning and after every change to
`src/`; a stale `dist/` serves the old code. `npm run typecheck` checks `src/` and `test/` together under
the same strict settings without writing anything.

The suite replaces `fetch` and reaches no host beyond loopback. It drives a real MCP client against the
server on an in-memory transport, spawns the real process to check what only a pipe shows (a stray
write to stdout, a process that outlives its client), and tests each tool for the exact request it
sends, the payload it returns, and the refusals it makes. The tests import the TypeScript sources
through `tsx` (`node --import tsx --test test/*.test.ts`), the spawned server included, so they never depend
on `dist/`. The two support files, `test/helpers.ts` and `test/surface.ts`, are imported and not run.

## Run

A client normally starts the server, so "running" it means pointing the client at it — see
[Connect a client](../guides/connect-a-client.md). To run it by hand:

```bash
GITHUB_API_KEY=<token> npm start
```

It then waits on stdin for a client. Press Ctrl-C to stop it, or close its stdin. Without the
variable it still starts; a tool then refuses and names the variable.

To serve HTTP on this machine only, a local install, give it the GitHub key and nothing else. It listens on
`127.0.0.1` and asks for no access token:

```bash
GITHUB_API_KEY=<token> npm run start:http   # http://127.0.0.1:3000/mcp, this machine only
```

To serve HTTP to other machines, a server install, also give it an access token for clients (at least 32
characters; on any address but loopback it will not start without one):

```bash
GITHUB_API_KEY=<token> MCP_AUTH_TOKEN=$(openssl rand -hex 32) npm run start:http   # port 3000, all interfaces
```

See [Set up HTTP with two tokens](../guides/setup-http-token.md). The two flags:

```bash
node dist/index.js --help       # usage, on stderr
node dist/index.js --version    # name and version, on stderr
```

## Inspect it

```bash
GITHUB_API_KEY=<token> npm run inspect
```

opens the MCP Inspector against the stdio server, so you can list the tools and call one by hand.
This is the quickest way to see the surface a fresh process would build.

## stdout belongs to the protocol

On stdio, stdout is the JSON-RPC channel. One stray write corrupts the stream, and the client
reports a JSON parse error that points at the payload rather than at the logging. Nothing in `src/`
writes to stdout; diagnostics go to stderr, which is why `--help` and `--version` print there.

## Tool changes require a restart

The registry is read once, at startup. Editing `src/controllers/` has no effect on a running process:
restart the client or the server.

## Related pages

* [Environment](env.md) — the variables.
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
* [Docker](docker.md) — the same server in a container.
* [Architecture](../information/architecture.md)
