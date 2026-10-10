# github

An MCP server that gives an LLM access to **GitHub** — repositories, branches, files, commits,
tags, releases, issues, pull requests and Actions workflows — through the GitHub REST API.

- **Server id:** `lxagents-github`
- **Package:** `@lxagents-mcp/github` — private; not published to npm.
- **Transport:** stdio by default: the client starts the server as a subprocess and talks to it through
  a pipe. HTTP with `MCP_TRANSPORT=http`, where every request carries the server's access token as
  `Authorization: Bearer <token>`; it will not start without one, listens on port 3000 on all interfaces
  unless told otherwise, and needs TLS in front before it faces a network.
- **Surface:** one tool per module in `src/tools/`, named by what it acts on and what it does
  (`repo_create`, `branch_list`, `pr_merge`, …). Every tool that acts on a repository takes one
  `repo` argument, `owner/name`. The list is in [MCP surface](wiki/reference/mcp-surface.md).
- **Tokens:** two, and never arguments. `GITHUB_API_KEY` is the GitHub token every call is made with, on
  both transports, read when a tool is called. `MCP_AUTH_TOKEN` is the access token a client presents to
  the HTTP transport (at least 32 characters, never sent to GitHub); stdio does not read it.
- **Requirements:** Node >= 20. ESM, no build step.

## Key features

- **One module, one tool.** A tool is a file in `src/tools/` named after it; the registry finds it
  and nothing else needs editing. A test holds files and tools together in both directions.
- **One place builds URLs.** A repository, a branch or tag name and a file path are validated and
  percent-encoded in `src/api.js`, so an argument cannot steer a call at another endpoint.
- **The key never leaves its header.** It is not an argument, it is read per call, and it is
  removed from every message the server can produce — refusals and crashes alike. Over HTTP a request
  without the access token is refused before anything is read, and the access token is never sent to GitHub.
- **Honest annotations.** Every tool says whether it reads, creates, changes or destroys, and a test
  holds it to that, so a client can ask a person first before anything destructive. `repo_delete`
  also refuses unless it is told the repository's full name.
- **Failures that say what to do next.** A rejected token, a missing permission, a rate limit with
  its reset time, a name already taken: each is a sentence, not a status code.
- **Bounded output.** Lists page at up to 100; a file, a log or a diff is cut at a stated limit and
  says it was cut.
- **Nothing leaves the machine in the tests.** `fetch` is replaced and any request a test did not
  expect fails it; the one exception, a redirect check, talks to two servers on loopback.

## Quick start

```bash
npm install
npm test
```

Serve it to an MCP client over stdio, with the GitHub key in the environment:

```bash
GITHUB_API_KEY=<token> npm start
```

Or over HTTP, where clients present an access token of your making (see
[Set up HTTP with two tokens](wiki/guides/setup-http-token.md)):

```bash
GITHUB_API_KEY=<token> MCP_AUTH_TOKEN=$(openssl rand -hex 32) npm run start:http   # port 3000, all interfaces
```

Or drive it by hand with the MCP Inspector:

```bash
GITHUB_API_KEY=<token> npm run inspect
```

Full instructions are in [`wiki/environments/setup.md`](wiki/environments/setup.md).

## Connect it

Add it to the client's configuration, with the GitHub key in the `env` block, then **restart the
client**. For Claude Desktop, in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "lxagents-github": {
      "command": "node",
      "args": ["/absolute/path/to/github/src/index.js"],
      "env": { "GITHUB_API_KEY": "<your GitHub token>" }
    }
  }
}
```

**Registering a server does not reach a session that is already running.** A client loads its
connector list at session start, so a server added mid-session reports healthy and is still absent
from the tool surface until the session restarts. See
[Connect a client](wiki/guides/connect-a-client.md) for Claude Code and what the token needs.

## Documentation

- [Overview](wiki/information/overview.md) — what this serves and why it is its own server.
- [Architecture](wiki/information/architecture.md) — how a call becomes a request and a result.
- [MCP surface](wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
- [Local setup](wiki/environments/setup.md) — running and testing it.
- [Docker](wiki/environments/docker.md) — the image, which has not yet been built.
- [Environment](wiki/environments/env.md) — the variables, and what the token needs.
- [Connect a client](wiki/guides/connect-a-client.md) — Claude Desktop and Claude Code over stdio.
- [Set up HTTP with two tokens](wiki/guides/setup-http-token.md) — running it as a service behind an access token, and connecting a client.
- [Smoke test](wiki/guides/smoke-test.md) — the first live run, which has not been done yet.
- [Security model](wiki/security/security-model.md) — the token, the surfaces that touch it, and
  what is deliberately not protected.

## Working with agents

This repository follows the organization's shared agent instruction set, read through a connector
and never copied here. Start at [`AGENTS.md`](AGENTS.md).

## License

MIT — see [`LICENSE`](LICENSE).
