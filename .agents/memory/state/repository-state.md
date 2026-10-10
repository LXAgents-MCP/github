---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/github — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Version `0.2.0`.** `package.json` and the lockfile carry it, and the release log is
`wiki/logs/0/2/0/CHANGELOG.md`; the first release, `0.1.0`, is `wiki/logs/0/1/0/CHANGELOG.md`. 0.2.0 added the
HTTP transport and renamed the token variable to `MCP_AUTH_TOKEN`. No git tag was made, by decision (D3 in
`tasks/forge-servers.md`, D8 in `tasks/http-token-auth.md`).

**What it is.** An MCP server, stdio by default and HTTP with a bearer token, `lxagents-github`, over the GitHub REST API at `api.github.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/tools/`. The full list is pinned in `test/surface.js` and
described in `wiki/reference/mcp-surface.md`: repositories, branches, files and directories, commits,
tags and releases, issues, pull requests and Actions workflows. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_github_repo`.

**Token.** The GitHub token: `MCP_AUTH_TOKEN` over stdio, read when a tool is called, or the bearer token
of each request over HTTP, which never reads it. `MCP_TRANSPORT`, `PORT` and `HOST` are the only other variables, and
the only flags are `--help` and `--version`.

**Verified.** The suite (299 tests) replaces `fetch` and reaches no host beyond loopback. Every request shape is
checked against GitHub's documentation and against mocks. The production-only dependency tree the image
would install serves every tool over a real stdio pipe, and the HTTP transport is tested over real
loopback sockets, including concurrent requests with different tokens and a real process.

**Not verified.** **Nothing has been exercised against live GitHub:** the environment the work was done
in could not reach `api.github.com` with a token. The write tools are the ones to prove first, and
`wiki/guides/smoke-test.md` is the walk and the list of behaviours the documentation leaves open. The
`Dockerfile` has never been built: there was no Docker daemon. The HTTP transport has not
been reached from another machine, behind a proxy, or with a real GitHub token.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport (decision
`http-token-transport.md`). No GitHub Enterprise.
Not published to npm (`private: true`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
