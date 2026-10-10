# Environment

**One variable, and it is a secret.**

| Variable | Default | What it does |
|---|---|---|
| `LXAGENTS_MCP_GITHUB_API_KEY` | none | The GitHub token every tool uses. Read when a tool is called, trimmed, and never taken as an argument. |

No other variable is read, there is no configuration file, and the only flags are `--help` and
`--version`. A second variable that takes a secret is a decision for the owner, not an
implementation detail: it widens what a leaked environment gives away.

## Why it is read per call

A server without a token still starts. A tool then refuses with a message that names the variable
and says where to put it, and every tool that does not need a token is unaffected. Reading at
startup would make a missing token a crash the client reports as "server failed to start", which
hides the cause.

## Where to put it

In the `env` block of this server's entry in the client's configuration, then restart the client:

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

Keep it out of the repository, the command line and any file that is committed. It is never logged,
never echoed in an error and never put in a result; the test suite asserts that neither a real-looking
token nor one GitHub echoes back reaches the output. `.env` and `.env.*` are gitignored.

## What the token needs

GitHub decides what each call may do from the token, not from this server, so the token is the real
limit on what an agent can change. Give it only what the tools you will use need: a **fine-grained
token** limited to the repositories involved is the least privilege GitHub offers, and a classic
token's `repo` scope is the broadest.

| Tools | Classic token | Fine-grained token |
|---|---|---|
| `repo_create` | `repo` | Administration set to write — and only where GitHub lets a fine-grained token create repositories. On a 403, use a classic token. |
| `repo_read`, `repo_search` | `repo` for private repositories, `public_repo` for public ones | Metadata set to read, which every fine-grained token has |
| `repo_update` | `repo` | Administration set to write |
| `repo_delete` | `delete_repo` | Administration set to write. Either way the token's owner must be an admin of the repository, and an organization can forbid its members to delete. |
| `repo_fork` | `repo` | GitHub documents no fine-grained permission for this endpoint; use a classic token if a fine-grained one is refused. |

The permissions are listed from GitHub's REST documentation. When a token is short of one, GitHub's
refusal says which permission the endpoint accepts and this server repeats it, so the refusal is the
authority if it disagrees with this table.

## What configures the server instead

The things that are constants, in `src/config.js`, because making them settings would be a change
to a public surface: the API base (`https://api.github.com`), the pinned API version, the 30 second
request timeout, the 20 MB ceiling on an answer, the 30 000 character ceiling on a result, and the
page sizes.

## Related pages

* [Security model](../security/security-model.md) — where the token can and cannot go.
* [Connect a client](../guides/connect-a-client.md)
