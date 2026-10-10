# Environment

**Five variables, and two of them are secrets.** One is the GitHub key, which every tool uses; one is the
access token that guards the HTTP transport; three choose and place the HTTP transport. A client that spawns
the server over stdio sets only the key.

| Variable | Default | What it does |
|---|---|---|
| `GITHUB_API_KEY` | none | The GitHub token every tool uses, **on both transports**. Read when a tool is called, trimmed, and never taken as an argument. |
| `MCP_AUTH_TOKEN` | none, **required for HTTP** | The server's own access token: what a client must send as `Authorization: Bearer <token>` to use the HTTP transport. At least 32 characters. **HTTP will not start without it.** stdio never reads it, and it is never sent to GitHub. |
| `MCP_TRANSPORT` | `stdio` | Which transport `src/index.js` speaks: `stdio`, or `http` (`streamable-http` is accepted too). Anything else exits 2. |
| `PORT` | `3000` | The port the HTTP transport listens on, the same as `shared-instruction`. A value that is not a port exits 2. The GitLab server also defaults to `3000`: set `PORT` to run both on one machine. |
| `HOST` | `0.0.0.0` | The interface it listens on: every one, as `shared-instruction` does. `HOST=127.0.0.1` keeps it to this machine. The process holds a GitHub key, so what guards it is `MCP_AUTH_TOKEN`; see [Security model](../security/security-model.md). |

There is no configuration file, and the only flags are `--help` and `--version`. A further variable that takes
a secret is a decision for the owner, not an implementation detail: it widens what a leaked environment gives
away. `LXAGENTS_MCP_GITHUB_API_KEY` was the variable before `0.2.0` and is gone; in `0.2.0` the GitHub token was
`MCP_AUTH_TOKEN`, which now means something else (see [If you used 0.2.0](../guides/setup-http-token.md#if-you-used-020)).

## Two secrets, two jobs

| | `GITHUB_API_KEY` | `MCP_AUTH_TOKEN` |
|---|---|---|
| Direction | server → GitHub | client → server |
| Used on | stdio and HTTP | HTTP only |
| Set in | The server's environment: the `env` block of its entry in the client's configuration over stdio, the process environment over HTTP | The server's environment, and the client's `headers` for its HTTP entry |
| Who sees it | Only the server | The server and each client you give it to |
| Missing | The server starts; a tool refuses and names the variable | HTTP refuses to start |

The access token says "this client may use the server"; the key is what the server then does GitHub's work
with. A client never sees the key, and the access token is never sent to GitHub.
[Set up HTTP with two tokens](../guides/setup-http-token.md) walks through it.

**`MCP_AUTH_TOKEN` is also the name `shared-instruction` uses, for its own token.** Give each server its own
value: two servers on one value let whoever holds it use both.

## Why the key is read per call

A server without the key still starts. A tool then refuses with a message that names the variable and says
where to put it, and every tool that does not need it is unaffected. Reading at startup would make a missing
key a crash the client reports as "server failed to start", which hides the cause. The access token is the
opposite on purpose: HTTP without it would be an open door, so that is the one missing secret that stops the
server.

## Where to put them

The key, over stdio, in the `env` block of this server's entry in the client's configuration, then restart the
client:

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

Over HTTP the server is started by you, with both variables in its environment, and each client sends the access
token in the `headers` of its entry; see [Set up HTTP with two tokens](../guides/setup-http-token.md).

Under Docker the `env` block reaches the `docker` process and `-e GITHUB_API_KEY` passes the key into the
container; for HTTP add `-e MCP_AUTH_TOKEN`. See [Docker](docker.md).

Keep both out of the repository, the command line and any file that is committed. Neither is ever logged, echoed
in an error or put in a result; the test suite asserts that neither a real-looking key nor one GitHub echoes back
reaches the output, and that the access token never reaches GitHub. `.env` and `.env.*` are gitignored.

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
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
