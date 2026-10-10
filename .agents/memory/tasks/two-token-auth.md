---
name: memory-tasks-two-token-auth
description: Separating the token a client presents to the GitHub MCP server (MCP_AUTH_TOKEN) from the key the server uses against GitHub (GITHUB_API_KEY); released as 0.3.0.
---

# Two tokens — GitHub

## 2026-10-10 — in progress

**Goal.** Two secrets with two jobs. One authenticates a client to this server; the other authenticates this
server to GitHub. 0.2.0 had one: over HTTP the client's bearer token was the GitHub token itself, and over stdio
`MCP_AUTH_TOKEN` held it.

**Objective.** Over HTTP every route but `/healthz` needs `Authorization: Bearer <MCP_AUTH_TOKEN>`, where
`MCP_AUTH_TOKEN` is a secret of at least 32 characters in the server's environment, compared in constant time;
HTTP will not start without it, as in `shared-instruction`. Every GitHub call, over stdio and over HTTP, is made
with `GITHUB_API_KEY`, read per call from the server's environment. The client's bearer is never forwarded and
never used against GitHub; stdio never reads `MCP_AUTH_TOKEN`. `npm test` is green and the wiki says how to set
both up.

**Detail.** Edit this repository only; `LXAgents-MCP/gitlab` gets the same change in its own record after this one,
with `GITLAB_API_KEY`. The 45 tools do not change. The reference is `shared-instruction`'s `src/auth.js`. The names
`GITHUB_API_KEY` and `GITLAB_API_KEY` are the owner's. This record supersedes `http-token-auth.md` in part.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/two-token-auth-plan` | |
| 2 | Separate the server token from the GitHub key | `feat/two-token-auth` | |
| 3 | Setup guide and corrected documentation | `docs/two-token-setup` | |
| 4 | Release | `release/0.3.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by a closing commit
once the pull requests exist.

## Decisions

The owner asked for the change and the names, and said to start after the plan. Where a choice was open the
recommended option was taken, per the standing instruction.

| # | Decision |
|---|---|
| T1 | Two secrets. `MCP_AUTH_TOKEN` is the server's own secret and guards the HTTP door; `GITHUB_API_KEY` is the GitHub token. Replaces D1 (the bearer token as the GitHub token) and D3 (HTTP never reads the environment) of `http-token-auth.md`. Declined: keeping the passthrough as an option, which is two ways to authenticate and a second path to test. |
| T2 | `MCP_AUTH_TOKEN` must be at least 32 characters and HTTP refuses to start without it, copied from `shared-instruction`. Replaces D4, which was about a token the server did not own; a secret the operator generates can have a floor. |
| T3 | stdio needs no client authentication, because a pipe has no header, and never reads `MCP_AUTH_TOKEN`. It reads `GITHUB_API_KEY`. |
| T4 | A missing `GITHUB_API_KEY` does not stop either transport. A tool refuses and names the variable; the HTTP process also says so on stderr at start. Reading per call is unchanged. |
| T5 | The bearer a client sends is compared and discarded. It is never sent to GitHub, never logged and never put in a result. The GitHub key is never taken from a request. |
| T6 | The comparison hashes both values with SHA-256 and compares the digests in constant time, so neither length nor a correct prefix shows in the timing. |
| T7 | The release is `0.3.0`: both variable names change again, which breaks 0.2.0 registrations. **Not approved by the owner in so many words**; chosen under the standing instruction and stated to them with the plan. No git tag, nothing published. |
| T8 | The name `MCP_AUTH_TOKEN` now means here what it means in `shared-instruction`. D10's hazard (a value meant for one server inherited by the other and sent to GitHub) is gone for stdio, which does not read it. What remains is two HTTP servers given one value, which the documentation advises against. |
| T9 | Consequence, recorded because the earlier decision rested on it: every holder of `MCP_AUTH_TOKEN` acts as the owner of `GITHUB_API_KEY`, `repo_delete` included, and there is no identity per caller. Mitigations: give the key the least scope that works, bind loopback by default, require a 32-character secret, terminate TLS in front, and rotate by restart. |

## Task entries

### Task 1 — chore/two-token-auth-plan

Landed: this record and its row in `memory-index.md`. The first commit of the branch fills the `PR` column of
`http-token-auth.md` for 0.2.0, which was left for a closing commit. Nothing outside `.agents/` changes in this
task. The `PR` column here is filled by a closing commit, not here, so no later branch needs a rebase.

Baseline before any change: `npm test` ran 299 passing and none failing.

### Task 2 — feat/two-token-auth

Landed. Over HTTP every route but `/healthz` now needs `Authorization: Bearer <MCP_AUTH_TOKEN>`. The check comes
before the body is parsed and before a path is matched, answers 401 in the JSON-RPC envelope with
`WWW-Authenticate: Bearer realm="mcp"`, and adds `error="invalid_token"` when a credential was sent and did not
match (T6: SHA-256 digests, `timingSafeEqual`). HTTP refuses to start without the token or with fewer than 32
characters, exits 1, names the variable and shows `openssl rand -hex 32`, and listens on nothing (T2). With a
valid token, 404, 405, 400 and 413 are as in 0.2.0.

Every GitHub call, on both transports, is made with `GITHUB_API_KEY` (T1, T4, T5): `requireToken` reads it per
call, and a missing key refuses the tool naming the variable. The HTTP process says so at start but runs. The
client's bearer is compared and dropped; a test searches the whole request to GitHub for it and finds nothing.
stdio is unchanged except for the variable's name and never reads `MCP_AUTH_TOKEN` (T3).

Removed: the per-request token, `AsyncLocalStorage`, `enterHttpMode` and the rule that HTTP ignores the
environment, all of which existed because the bearer was the GitHub token. `src/auth.js` is now two small
groups, the door and the key. `src/config.js` keeps `TOKEN_ENV` for the GitHub key's name, now
`GITHUB_API_KEY`, and adds `AUTH_ENV`, `MIN_AUTH_TOKEN_LENGTH`. The 512-character cap on a presented credential is
gone with the passthrough; any credential that is not the token is simply wrong. The decision is recorded in
`.agents/memory/decisions/two-token-auth.md`, with the cost it carries (T9), and `http-token-transport.md` is
marked partly superseded.

Tests: 307 passing, none failing (299 before). `test/auth.test.js` and `test/http.test.js` are rewritten for the
two secrets: the header parser, the floor, the constant-time comparison against near-misses (prefix, suffix,
longer, case, empty, the GitHub key), the 401 cases, the exact `/healthz` exemption, the key used against
GitHub with the access token absent from the request, no upstream call without the access token or without the
key, 8 concurrent calls, neither secret in a response or the log, startup refusals in-process and from a real
process, and a real process that serves, ignores nothing it should read, and stops on `SIGTERM`. Making the
comparison always true fails five tests; allowing an empty token fails the two startup tests, which was
confirmed. By hand against a running server: no token exits 1 with the message, `/healthz` answers, no bearer
and a wrong bearer are 401 with the right challenge, a right bearer lists 45 tools, and a tool without
`GITHUB_API_KEY` is refused naming it.

Left for task 3: the setup guide and every page that says the bearer is the GitHub token. The rules in
`.agents/rules/repository.md` and the security procedure still describe the passthrough until task 3.

### Task 3 — docs/two-token-setup

Landed. `wiki/guides/setup-http-token.md` is rewritten around the two secrets: a table of what each is and who
holds it; creating the GitHub key and the access token (`openssl rand -hex 32`, one per server); starting the
server with both; curl checks including the 401 cases; registering a client with only the access token, using a
variable of the client's own (`${LXAGENTS_GITHUB_ACCESS}`); TLS; the container with both passed by name; a section
on what the access token does and does not mean for who can do what (T9); a migration table from 0.2.0; and a
troubleshooting table. Its title and every link to it say "two tokens".

Corrected, because they said the bearer was the GitHub token or named `MCP_AUTH_TOKEN` as the GitHub token:
`env.md` (five variables, a table of the two secrets side by side), `setup.md`, `connect-a-client.md`,
`docker.md`, `architecture.md` ("The two secrets"), `overview.md`, `mcp-surface.md`, `security-model.md` (the
surfaces, secrets, deployment posture with its stated cost, verification rows, not-a-finding items),
`smoke-test.md` (an HTTP walk of nine steps), `README.md`, the intro of `AGENTS.md`, and the comments of the
`Dockerfile`. On the agent side: `rules/repository.md`, `wiki/context/repository-map.md`,
`wiki/security/security-boundaries.md`, `memory/state/repository-state.md` and `index/project-wiki-index.md`. The
released `0.2.0` changelog and its logs-index row are left as written; corrections go in the next version.

Checked: no relative link in any `.md` file is broken. Code is untouched in this task, so `npm test` is as task 2
left it.
