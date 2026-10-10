# Connect a Client

How to register this server with an MCP client and give it a token. Four steps; the third is the one
people forget.

## 1. Create a token

On GitHub, create a personal access token. Prefer a **fine-grained** token limited to the repositories
the agent should touch; use a classic token only where GitHub does not allow the fine-grained one for
what you need (creating a repository in a personal account is the usual case). What each tool needs
is in [Environment](../environments/env.md#what-the-token-needs). Treat the token like a password: it
can do whatever you granted it.

## 2. Register the server

The repository is private and the package is not published, so the client runs the server by path.
Clone the repository, run `npm install` once, then add the server to the client.

**Claude Desktop** — edit `claude_desktop_config.json`:

* Windows: `%APPDATA%\Claude\claude_desktop_config.json`
* Mac: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "lxagents-github": {
      "command": "node",
      "args": ["/absolute/path/to/github/src/index.js"],
      "env": { "LXAGENTS_MCP_GITHUB_API_KEY": "<your GitHub token>" }
    }
  }
}
```

**Claude Code** — from a terminal:

```bash
claude mcp add lxagents-github \
  --env LXAGENTS_MCP_GITHUB_API_KEY=<your GitHub token> \
  -- node /absolute/path/to/github/src/index.js
```

Or commit a project `.mcp.json` that reads the token from the environment instead of holding it:

```json
{
  "mcpServers": {
    "lxagents-github": {
      "command": "node",
      "args": ["/absolute/path/to/github/src/index.js"],
      "env": { "LXAGENTS_MCP_GITHUB_API_KEY": "${LXAGENTS_MCP_GITHUB_API_KEY}" }
    }
  }
}
```

**In a container** instead of on the host: the same entry with `docker` as the command. See
[Docker](../environments/docker.md).

## 3. Restart the client

**Registering a server does not reach a session that is already running.** The client loads its
connector list at session start, so a server added mid-session reports healthy and is still absent
from the tool surface until the session restarts. That is the most common cause of "the tools are
not there", and it looks like a broken server rather than a stale session.

## 4. Check that it is there

Ask the client to list the server's tools, or run the MCP Inspector (`npm run inspect`). You should
see the tools in [MCP surface](../reference/mcp-surface.md), all taking a `repo` as `owner/name`
except `repo_create` and `repo_search`.

Then ask for something harmless and reversible, such as creating a private repository with a
throwaway name:

> Create a private GitHub repository called `scratch-delete-me` in my account.

A rejected token comes back as a message naming `LXAGENTS_MCP_GITHUB_API_KEY`; a token that lacks a
permission comes back as a 403 that says so.

## Related pages

* [Environment](../environments/env.md)
* [Local setup](../environments/setup.md)
* [Docker](../environments/docker.md)
