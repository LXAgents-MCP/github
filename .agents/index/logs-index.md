---
name: logs-index
description: This repository's release history, newest first — what changed and what clients must do.
---

# Logs Index

**Scope:** `wiki/logs/`
**Parent:** [`root-index.md`](root-index.md)

A client picks up a change when it next starts the server, so the **Clients must** column is the
only notice it gets. It is never left blank.

## Versions

| Version | Date | Summary | Clients must |
|---|---|---|---|
| [`0/3/0`](../../wiki/logs/0/3/0/CHANGELOG.md) | 2026-10-10 | **Two secrets.** `MCP_AUTH_TOKEN` is now the server's own access token: over HTTP a client must send it as `Authorization: Bearer <token>` (at least 32 characters; HTTP will not start without it). `GITHUB_API_KEY` is the GitHub token, used for every call on stdio and HTTP, never supplied by a client. **Breaking:** `MCP_AUTH_TOKEN` held the GitHub token in 0.2.0. Everyone holding the access token acts as the key's owner. The 45 tools are unchanged. Still nothing run against live GitHub. | **Over stdio, move the GitHub token from `MCP_AUTH_TOKEN` to `GITHUB_API_KEY`** (the `env` block, or `docker run -e`). **To serve HTTP, start it with `GITHUB_API_KEY` and a new `MCP_AUTH_TOKEN`** of 32+ characters (`openssl rand -hex 32`), one per server, and register clients with `Authorization: Bearer <MCP_AUTH_TOKEN>`, not a GitHub token. Restart the client. |
| [`0/2/0`](../../wiki/logs/0/2/0/CHANGELOG.md) | 2026-10-10 | **HTTP, with the GitHub token as the bearer token.** `MCP_TRANSPORT=http` serves `POST /mcp` and `GET /healthz` on `127.0.0.1:3001`; every request carries `Authorization: Bearer <GitHub token>`, the server stores no secret and HTTP never reads the environment. The token variable is renamed `MCP_AUTH_TOKEN`; `LXAGENTS_MCP_GITHUB_API_KEY` is removed. The 45 tools are unchanged. Still nothing run against live GitHub. | **Rename `LXAGENTS_MCP_GITHUB_API_KEY` to `MCP_AUTH_TOKEN`** in the `env` block of the server's entry (and in `docker run -e`), and set it only there, never exported in a shell, since `shared-instruction` uses the same name for a different secret. To use HTTP, register the URL with an `Authorization: Bearer` header and run `npm run start:http`. Restart the client. |
| [`0/1/0`](../../wiki/logs/0/1/0/CHANGELOG.md) | 2026-10-10 | **The first release.** A stdio MCP server for GitHub in 45 tools: repositories, branches, files, commits, tags, releases, issues, pull requests and Actions workflows. `repo_create` is `create_github_repo` moved from `cli` 2.1.0 and renamed, otherwise unchanged. Nothing has run against live GitHub, and the image has never been built. | **Create a token and register the server** with `LXAGENTS_MCP_GITHUB_API_KEY` in its `env` block (clone, `npm install`, `node src/index.js`); restart the client; run the smoke test before relying on a write tool. **If you called `create_github_repo` on `cli`, call `repo_create` here** — the variable has the same name but belongs in this server's `env` block. |

## Maintenance

* Newest version first, one row per version directory.
* A new version directory is a version claim and needs the owner's approval.
* Never edit a released log to change history; corrections go in the next version.
