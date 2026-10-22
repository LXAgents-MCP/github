---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/github — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Version `0.4.0`.** `package.json` and the lockfile carry it, and the release log is
`wiki/logs/0/4/0/CHANGELOG.md`; earlier releases are `0/3/0`, `0/2/0` and `0/1/0`. 0.4.0 moved the HTTP defaults to
`shared-instruction`'s, port 3000 on every interface. No git tag was made, by decision (D3 in
`tasks/forge-servers.md`, D8 in `tasks/http-token-auth.md`, T7 in `tasks/two-token-auth.md`, H3 in
`tasks/http-defaults.md`).

**What it is.** An MCP server, stdio by default and HTTP behind an access token, `lxagents-github`, over the GitHub REST API at `api.github.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/controllers/`. The full list is pinned in `test/surface.js` and
described in `wiki/reference/mcp-surface.md`: repositories, branches, files and directories, commits,
tags and releases, issues, pull requests and Actions workflows. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_github_repo`.

**Tokens.** Two. `GITHUB_API_KEY` is the GitHub token, read when a tool is called, on both transports.
`MCP_AUTH_TOKEN` is the access token HTTP requires of a client (32 characters or more; HTTP exits 1 without it) and
is never read over stdio or sent to GitHub. `MCP_TRANSPORT`, `PORT` (default 3000) and `HOST` (default `0.0.0.0`) are the only other variables, and the
only flags are `--help` and `--version`.

**Verified.** The suite (309 tests) replaces `fetch` and reaches no host beyond loopback. Every request shape is
checked against GitHub's documentation and against mocks. The production-only dependency tree the image
would install serves every tool over a real stdio pipe, and the HTTP transport is tested over real
loopback sockets, including near-miss access tokens, the access token's absence from every request to GitHub, and real processes
that refuse to start open and that serve and stop on `SIGTERM`.

**Not verified.** **Nothing has been exercised against live GitHub:** the environment the work was done
in could not reach `api.github.com` with a token. The write tools are the ones to prove first, and
`wiki/guides/smoke-test.md` is the walk and the list of behaviours the documentation leaves open. The
`Dockerfile` has never been built: there was no Docker daemon. The HTTP transport has not
been reached from another machine, behind a proxy, or with a real GitHub key.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport, and no identity per caller (decision
`two-token-auth.md`). No GitHub Enterprise.
Not published to npm (`private: true`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
