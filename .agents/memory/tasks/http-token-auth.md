---
name: memory-tasks-http-token-auth
description: Giving the GitHub MCP server an HTTP transport that requires a bearer token, and renaming its one variable to MCP_AUTH_TOKEN; released as 0.2.0.
---

# Token over HTTP — GitHub

## 2026-10-10 — in progress

**Goal.** A client can reach the GitHub server the way it reaches `shared-instruction` over HTTP: a
required bearer token in the `Authorization` header, set in the client's configuration, instead of a
secret that has to be in the server's own environment.

**Objective.** `MCP_TRANSPORT=http node src/index.js` serves `POST /mcp` and `GET /healthz`. Every route
but `/healthz` needs `Authorization: Bearer <GitHub token>`, answers 401 without it, and uses that token
for the GitHub calls of that request only. The one variable is `MCP_AUTH_TOKEN`, the name
`shared-instruction` uses for its token; stdio reads the GitHub token from it, HTTP never does. The old
`LXAGENTS_MCP_GITHUB_API_KEY` is gone everywhere. `npm test` is green and a wiki page says how to set it up.

**Detail.** Edit this repository only; `LXAgents-MCP/gitlab` gets the same change in its own record, after
this one. The 45 tools do not change. No new dependency: the SDK's streamable-HTTP transport runs on
`node:http`. The reference is `shared-instruction`'s `src/auth.js` and `wiki/environments/env.md`. The
version bump to `0.2.0` was approved with the plan.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/http-token-auth-plan` | |
| 2 | HTTP transport with a bearer token | `feat/http-token-auth` | |
| 3 | Setup guide and corrected documentation | `docs/http-token-setup` | |
| 4 | Release | `release/0.2.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by a closing
commit once the pull requests exist.

## Decisions the owner approved with the plan

| # | Decision |
|---|---|
| D1 | Over HTTP the client's bearer token is the GitHub token itself, passed through per request. The server stores no secret. Declined: a separate server access token beside a GitHub token in the environment, which is two secrets and still an environment. |
| D2 | stdio stays, because a pipe has no header to carry a token, and reads the GitHub token from `MCP_AUTH_TOKEN`. The old `LXAGENTS_MCP_GITHUB_API_KEY` is deleted, not kept as a second name. Declined: removing stdio, which breaks every registration and `docker run -i`. |
| D3 | HTTP never reads the environment variable. An environment token is not a fallback, or a caller with no token would act with the owner's. |
| D4 | The token is checked for shape only: not empty, no whitespace, at most 512 characters. `shared-instruction` refuses under 32, which would refuse a GitLab token (26 characters), so the floor is not copied. |
| D5 | The default bind is `127.0.0.1`, because what crosses the wire is a GitHub credential. `HOST` overrides it. `shared-instruction` binds `0.0.0.0`. |
| D6 | The variables are `MCP_TRANSPORT`, `PORT` and `HOST`, named as in `shared-instruction`. The default port is `3001`, so the three servers can run on one machine. |
| D7 | No dependency is added: `node:http` and the SDK's transport. Declined: Express. |
| D8 | The release is `0.2.0`, approved. No git tag is created and nothing is published without its own approval. |
| D9 | Standing instruction: where a choice is needed, use the recommended option and add it to this table. |
| D10 | The variable is named `MCP_AUTH_TOKEN` in both forge servers, by the owner's instruction, the name `shared-instruction` uses for a different secret. A value exported in a shell for one would be inherited by the other's stdio process and sent to GitHub, so the documentation says to set it only in a server's own `env` block. |

## Task entries

### Task 1 — chore/http-token-auth-plan

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes in this task.
The `PR` column is filled by a closing commit, not here, so no later branch needs a rebase.

Baseline before any change: `npm test` ran 279 passing and none failing.
