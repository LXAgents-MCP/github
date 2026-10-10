# 0.1.0

**Released:** 2026-10-10

The first release of `lxagents-github`, an MCP server that gives an LLM access to GitHub through
the REST API: repositories, branches, files, commits, tags, releases, issues, pull requests and
Actions workflows, in 45 tools. `repo_create` is the `create_github_repo` tool of `cli` 2.1.0,
moved here and renamed, with the same arguments and behaviour; `cli` drops its copy in its own
3.0.0.

**Nothing has run against live GitHub.** The environment this was built in could not reach
`api.github.com` with a token, so every request is checked against GitHub's documentation and against
mocks, and no more. The write tools — everything that creates, changes, merges or deletes — are the
ones to prove first. [Smoke test](../../../../guides/smoke-test.md) is the walk, and lists the
behaviours the documentation leaves open.

**The server is stdio only, with no network listener.** It makes outbound HTTPS requests, to
`api.github.com` and, to read a job's log, to the signed address GitHub redirects to; the token is
not sent there.

**Clients must, to use it:**

1. **Create a token.** Prefer a fine-grained token limited to the repositories involved; creating a
   repository in a personal account needs a classic token. What each tool needs is in
   [Environment](../../../../environments/env.md#what-the-token-needs).
2. **Register the server** in the client configuration with the token in its `env` block, as
   `LXAGENTS_MCP_GITHUB_API_KEY`: clone the repository, run `npm install` once, and point the client
   at `node src/index.js`. See [Connect a client](../../../../guides/connect-a-client.md). The package
   is private and is not published to npm.
3. **If you called `create_github_repo` on the `cli` server, call `repo_create` on this one.** The
   variable has the same name but must now be in this server's `env` block, not `cli`'s. Until `cli`
   3.0.0 lands both exist.
4. **Restart the client**, so it reloads the connector and lists the tools.
5. **Run the smoke test** against a repository you can throw away before relying on a write tool.

A server without the variable still starts; a tool then refuses with a message naming it.

## Added

- **Repositories:** `repo_create`, `repo_read`, `repo_update`, `repo_delete` (which refuses unless
  `confirm` repeats the repository's full name), `repo_fork` and `repo_search`.
- **Branches:** `branch_create`, `branch_read`, `branch_list`, `branch_delete` and `branch_merge`,
  which merges one branch into another directly, with no pull request.
- **Files and directories:** `file_read` (cut at 25,000 characters; binary files and files over 1 MB
  refused), `directory_list`, `file_create`, `file_update` and `file_delete`. Each write is one
  commit on a branch you name, and an update or delete reads the file's current sha unless you pass
  one, so a file that moved on is refused.
- **Commits:** `commit_read` (optionally with its diff, cut at 20,000 characters), `commit_list`
  and `commit_compare`.
- **Tags and releases:** `tag_check` (a missing tag is an answer, not an error), `tag_get`,
  `tag_get_all`, `tag_delete`, `release_create` (which makes the tag when it does not exist),
  `release_read`, `release_list` and `release_delete`. There is no separate tag-creating tool.
- **Issues:** `issue_open`, `issue_read`, `issue_comment`, `issue_close`, `issue_assign`,
  `issue_label` and `issue_lock`. GitHub silently drops labels, assignees and a milestone from a
  token without push access, so the tools report what was applied and name what was not.
- **Pull requests:** `pr_open` (from a branch, or `owner:branch` in a fork), `pr_read`, `pr_comment`,
  `pr_close`, `pr_merge`, `pr_review_request` and `pr_approve`. `pr_read` says when GitHub has not
  yet worked out whether a pull request can be merged, instead of answering no.
- **Workflows:** `workflow_list`, `workflow_run` (by dispatch, with inputs), `workflow_status` (a run by
  id, or the latest run of a workflow) and `workflow_logs` (the failed jobs' logs by default, cut to
  their ends, 20,000 characters in all).
- **One way to name things.** Every tool but `repo_create` and `repo_search` takes a single `repo`
  argument, `owner/name`. Lists take `page` and `per_page` and say whether another page follows.
  Each result is readable text with the same fields as structured content, and long content is cut at
  a stated limit and says so.
- **Honest annotations.** Every tool is marked as reading, creating, changing or destroying, and a
  test holds it to that, so a client can ask a person before anything destructive.
- **Failures that say what to do next:** a rejected token, a missing permission with GitHub's own
  hint, a rate limit with its reset time, a name already taken, a pull request that cannot be merged.
- **`--help` and `--version`**, which write to stderr and leave stdout to the protocol.
- **A `Dockerfile`** for a stdio image, which has never been built. See
  [Docker](../../../../environments/docker.md).
- **The server closes and exits on `SIGTERM` as well as `SIGINT`**, which the image needs.
- **Documentation:** the README, the wiki, and the repository's own agent instruction set, which
  consumes the shared set through the connector and copies none of it.
- **Tests:** 279 under `node --test`. `fetch` is replaced and an unexpected request fails the test; the
  one exception, a redirect check, talks to two servers on loopback only.

## Changed

- **`repo_create` is `create_github_repo` from `cli`**, renamed. Its arguments, defaults (private,
  with an initial README), results and refusal wording are unchanged.

## Security

- **The token is read from the environment at call time.** It is never an argument, so it does not
  appear in a prompt or a transcript, and a server without one still starts.
- **The token is never logged or returned.** It is removed from every message the server can produce,
  refusals and crashes alike, and a test makes GitHub, the network and a handler echo it.
- **One place builds URLs.** A repository, a branch or tag name, a file path and a workflow are
  validated and percent-encoded there, so an argument cannot steer a call at another endpoint, and no
  argument can choose the host.
- **A job's log arrives through a redirect to another host.** The runtime drops the `Authorization`
  header when a redirect changes host, and a test shows a second host receiving no token.
- **The token's permissions are the real limit.** GitHub enforces them, not this server: give it only
  what the tools in use need.
