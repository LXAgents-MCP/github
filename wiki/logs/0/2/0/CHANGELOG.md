# 0.2.0

**Released:** 2026-10-10

`lxagents-github` can now be reached over HTTP, and the GitHub token has a new name. The server
still speaks stdio by default; with `MCP_TRANSPORT=http` it serves `POST /mcp` and `GET /healthz`,
and every request carries the GitHub token as `Authorization: Bearer <token>`, the way a client
reaches `shared-instruction`. The 45 tools, their arguments and their results do not change.

**Over HTTP the bearer token is the GitHub token.** The server holds no secret of its own: it uses the
token on a request for the GitHub calls of that request and forgets it, so two clients with different
tokens act as two different GitHub users. HTTP never reads the environment, so a request with no
token is refused and never run with the owner's.

**Nothing has run against live GitHub**, as in 0.1.0, and the HTTP transport has been exercised only on
loopback with `fetch` standing in for GitHub. [Smoke test](../../../../guides/smoke-test.md) now has an HTTP
walk as well as the tool walk.

**The server is no longer stdio only.** Over HTTP it listens on one port, on `127.0.0.1` unless `HOST` says
otherwise, because what crosses the wire is a GitHub credential. It does not terminate TLS: put a proxy
that does in front of it before reaching it from another machine. See
[Set up HTTP with a token](../../../../guides/setup-http-token.md).

**Clients must, to keep using it:**

1. **Rename the variable.** `LXAGENTS_MCP_GITHUB_API_KEY` is gone. In the `env` block of the server's entry,
   or in `docker run -e`, use **`MCP_AUTH_TOKEN`** with the same token. A server still given the old
   name starts, and the first tool that needs a token refuses and names the new one. Nothing else about
   a stdio registration changes.
2. **Set `MCP_AUTH_TOKEN` only in this server's own `env` block.** `shared-instruction` uses the same
   name for a different secret, so do not export it in a shell: a stdio process inherits its
   environment and would send that value to GitHub as if it were a GitHub token.
3. **To use HTTP, register the URL and a header** instead of a command: `claude mcp add --transport http
   lxagents-github http://127.0.0.1:3001/mcp --header "Authorization: Bearer <token>"`, with a
   variable of the client's own, not `MCP_AUTH_TOKEN`, where the header expands one. Then start the
   server with `npm run start:http`.
4. **Restart the client**, so it reloads the connector.

## Added

- **An HTTP transport,** selected by `MCP_TRANSPORT=http` (`streamable-http` is accepted too) or started
  directly with `npm run start:http`. `POST /mcp` serves the MCP protocol statelessly, one server per
  request; `GET /healthz` says who is there and needs no token. `PORT` (default `3001`) and `HOST`
  (default `127.0.0.1`) place it. A bad `MCP_TRANSPORT` or `PORT` exits 2.
- **A bearer token on every route but `/healthz`.** A missing or malformed header is a 401 in the JSON-RPC
  envelope with `WWW-Authenticate: Bearer realm="mcp"`; a credential over 512 characters or not
  printable ASCII adds `error="invalid_token"`. The check comes before the body is read and before the
  path is matched, so a caller without a token cannot make the server read 4 MB or find out which
  routes exist. With a token, an unknown path is 404, a method other than POST is 405 with `Allow: POST`,
  a body that is not JSON is 400 and one over 4 MB is 413.
- **`src/auth.js`,** the one place the token is chosen: the request's bearer token over HTTP, `MCP_AUTH_TOKEN`
  over stdio, and nothing else. A process that serves HTTP never consults the environment, even if a
  request's context is lost, and says on stderr, without the value, when `MCP_AUTH_TOKEN` is set and
  ignored.
- **`npm run start:http`.**
- **A setup guide,** [Set up HTTP with a token](../../../../guides/setup-http-token.md): creating the token,
  starting the server, checking it with curl, registering a client, TLS, containers, migrating from the
  old variable, and what each refusal means.
- **Tests:** 299 under `node --test`, 20 more than 0.1.0. They run the transport over real loopback sockets,
  check that 8 concurrent requests with different tokens never mix, that a leftover `MCP_AUTH_TOKEN`
  is ignored over HTTP, and that a real process answers and stops on `SIGTERM` with nothing on stdout.

## Changed

- **The token variable is `MCP_AUTH_TOKEN`,** replacing `LXAGENTS_MCP_GITHUB_API_KEY`, which is removed
  everywhere and not kept as a second name. The refusal for a missing token, the tool descriptions and
  the server's own instructions name the new variable, or the `Authorization` header over HTTP.
- **A rejected token is explained by where it came from.** Over HTTP GitHub's 401 names the `Authorization:
  Bearer` header; over stdio it names `MCP_AUTH_TOKEN`.
- **The documentation no longer says stdio only:** the wiki, the README, the `Dockerfile` comments and the
  repository's own agent rules describe two transports and where the token comes from in each.

## Security

- **HTTP never falls back to the environment's token.** A request without a token is a 401, not a call made
  as the owner. This is the guard, and a test fails if it is removed.
- **The token is still never logged or returned,** over HTTP as over stdio. It is removed from every message
  the server can produce, refusals and crashes alike, and the test suite makes GitHub, the network and
  a handler echo it.
- **The default bind is loopback,** because a GitHub token crosses the wire. `HOST=0.0.0.0` is for a
  container whose published port is bound to the host's loopback.
- **There is no `Host` allow-list, rate limit or TLS** in this server. GitHub limits each token on its
  own; terminate TLS in front. See [Security model](../../../../security/security-model.md).
- **No dependency was added.** The transport is the SDK's, on `node:http`.
