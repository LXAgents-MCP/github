# Environment

**Four variables, and exactly one of them is a secret.** One carries the GitHub token over stdio; three
choose and place the HTTP transport. A client that spawns the server over stdio sets only the first.

| Variable | Default | What it does |
|---|---|---|
| `MCP_AUTH_TOKEN` | none | The GitHub token every tool uses **over stdio**. Read when a tool is called, trimmed, and never taken as an argument. **HTTP never reads it.** |
| `MCP_TRANSPORT` | `stdio` | Which transport `src/index.js` speaks: `stdio`, or `http` (`streamable-http` is accepted too). Anything else exits 2. |
| `PORT` | `3001` | The port the HTTP transport listens on. Not a port exits 2. |
| `HOST` | `127.0.0.1` | The interface it listens on. Loopback by default, because what crosses the wire is a GitHub token. |

There is no configuration file, and the only flags are `--help` and `--version`. A second variable that
takes a secret is a decision for the owner, not an implementation detail: it widens what a leaked
environment gives away. `LXAGENTS_MCP_GITHUB_API_KEY` was the variable before `0.2.0` and is gone.

## Where the token comes from

| Transport | Source | Set in |
|---|---|---|
| stdio | `MCP_AUTH_TOKEN` | The `env` block of this server's entry in the client's configuration |
| HTTP | `Authorization: Bearer <token>` on every request | The client's configuration of its HTTP entry, under `headers` |

Over HTTP the bearer token **is** the GitHub token. The server stores no secret of its own, uses the
token for the GitHub calls of that one request, and does not read `MCP_AUTH_TOKEN` at all: a request
without a token is refused, never run with the environment's. [Set up HTTP with a token](../guides/setup-http-token.md)
walks through it.

**`MCP_AUTH_TOKEN` is also the name `shared-instruction` uses, for a different secret.** The two servers
are separate processes, so there is no clash as long as each gets its own value: set it in a server's
own `env` block, and do not export it in a shell, where a stdio process would inherit a value meant for
the other and send it to GitHub.

## Why it is read per call

A server without a token still starts. A tool then refuses with a message that names the variable
and says where to put it, and every tool that does not need a token is unaffected. Reading at
startup would make a missing token a crash the client reports as "server failed to start", which
hides the cause.

## Where to put it

Over stdio, in the `env` block of this server's entry in the client's configuration, then restart the
client:

```json
{
  "mcpServers": {
    "lxagents-github": {
      "command": "node",
      "args": ["/absolute/path/to/github/src/index.js"],
      "env": { "MCP_AUTH_TOKEN": "<your GitHub token>" }
    }
  }
}
```

Over HTTP, in the `headers` of the client's entry; see [Set up HTTP with a token](../guides/setup-http-token.md).

Under Docker the `env` block reaches the `docker` process and `-e MCP_AUTH_TOKEN` passes it into the
container for stdio; see [Docker](docker.md).

Keep it out of the repository, the command line and any file that is committed. It is never logged,
never echoed in an error and never put in a result; the test suite asserts that neither a real-looking
token nor one GitHub echoes back reaches the output, over stdio and over HTTP. `.env` and `.env.*` are
gitignored.

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
| `branch_read`, `branch_list` | `repo` | Contents set to read |
| `branch_create`, `branch_delete`, `branch_merge` | `repo` | Contents set to write |
| `file_read`, `directory_list` | `repo` | Contents set to read |
| `commit_read`, `commit_list`, `commit_compare` | `repo` | Contents set to read |
| `tag_check`, `tag_get`, `tag_get_all`, `release_read`, `release_list` | `repo` | Contents set to read |
| `tag_delete`, `release_create`, `release_delete` | `repo` | Contents set to write |
| `issue_read` | `repo` | Issues set to read (Pull requests set to read, for a pull request's number) |
| `issue_open`, `issue_comment`, `issue_close`, `issue_assign`, `issue_label`, `issue_lock` | `repo` | Issues set to write (Pull requests set to write, for a pull request's number) |
| `pr_read` | `repo` | Pull requests set to read |
| `pr_open`, `pr_comment`, `pr_close`, `pr_review_request`, `pr_approve` | `repo` | Pull requests set to write |
| `pr_merge` | `repo` | Pull requests set to write and Contents set to write |
| `workflow_list`, `workflow_status`, `workflow_logs` | `repo` | Actions set to read |
| `workflow_run` | `repo` | Actions set to write |
| `file_create`, `file_update`, `file_delete` | `repo`, plus `workflow` for a file under `.github/workflows/` | Contents set to write, plus Workflows set to write for a file under `.github/workflows/` |

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
* [Set up HTTP with a token](../guides/setup-http-token.md)
