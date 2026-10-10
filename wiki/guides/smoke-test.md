# Smoke Test

**Nothing in this repository has run against live GitHub.** Every request is checked against GitHub's
documentation and against mocks, which prove that the server sends what it means to send, not that
GitHub accepts it. This page is the first live run: a walk through every tool on a repository you can
throw away, then the behaviours the documentation leaves open, each with what to look for.

Do it once after installing, and again after a change to a request. **Use a throwaway repository and a
token you can revoke.**

## Before you start

* **A classic token** with the `repo`, `workflow` and `delete_repo` scopes. A fine-grained token cannot
  create a repository in a personal account, and GitHub documents none for forking; the rest of the walk
  also works with a fine-grained token limited to the scratch repository, with what
  [Environment](../environments/env.md#what-the-token-needs) lists.
* **The server registered, and the client restarted**, so that it lists the tools. See
  [Connect a client](connect-a-client.md).
* **Something public to fork**, such as `octocat/Hello-World`.

## The walk

The repository is `scratch-delete-me`; replace `<you>` with the token owner's login. Each row is one
call, and later rows use what earlier ones made.

| # | Tools and arguments | Expect |
|---|---|---|
| 1 | `repo_create` `name: scratch-delete-me`, `private: true`, `auto_init: true` | A private repository with a README and the default branch `main`. |
| 2 | `repo_read`, then `repo_update` `description: scratch`, then `repo_read` | The description is set; nothing else changed. |
| 3 | `repo_search` `query: scratch-delete-me in:name user:<you>` | The repository — or no match for a minute, while GitHub's search index catches up. |
| 4 | `file_create` `path: docs/a.md`, `content: one`, `message: add a`; `file_read`; `directory_list` `path: docs` | A commit; the same text and a blob sha back; `a.md` listed. |
| 5 | `file_create` the same path again | Refused, saying the file exists. |
| 6 | `file_update` with the sha from `file_read`; then again with the old sha | The first commits; the second is refused as stale (409 or 422). |
| 7 | `branch_create` `branch: feat/smoke`; `branch_read`; `branch_list` | A branch from `main` — the slash in its name is the point — with the same tip as `main`. |
| 8 | `file_create` `path: docs/b.md`, `branch: feat/smoke` | The file is on `feat/smoke` and not on `main`. |
| 9 | `commit_list` `ref: feat/smoke`; `commit_read` with `diff: true` on the new commit; `commit_compare` `base: main`, `head: feat/smoke` | The branch's commits, newest first, the new one on top; a diff as plain text; ahead by one, behind by none, one changed file. |
| 10 | `issue_open` with `labels: [bug]`, `assignees: [<you>]`; `issue_read` with `comments: true`; `issue_comment`; `issue_label`; `issue_assign` with `action: remove`; `issue_lock` then `issue_lock` `locked: false`; `issue_close` `reason: not_planned` | Each step is visible in `issue_read`. Label and assignee are applied, and `not_applied` is empty. |
| 11 | `pr_open` `head: feat/smoke`, `title: Smoke`; `pr_read` with `files: true`; `pr_comment`; `pr_approve` | A pull request. `pr_read` shows `docs/b.md`, and says whether it can be merged or that GitHub is still working it out. `pr_approve` is refused, because you opened it. |
| 12 | `pr_review_request` for a collaborator, if the repository has one | The reviewer is listed in `pr_read`. Skip it otherwise. |
| 13 | On `main`, `file_create` `path: .github/workflows/smoke.yml` with the workflow below | A commit. This is why the token needs `workflow`. |
| 14 | `workflow_list`; `workflow_run` `workflow: smoke.yml`, `inputs: {word: hi}`; `workflow_status` `workflow: smoke.yml`, until `completed` | `Smoke` listed, perhaps after a few seconds. A run id, or a message that GitHub did not say which run. Status ends `completed`, conclusion `failure`, with job `fail` and its `failed_step`. |
| 15 | `workflow_logs` `run_id: <the run>` | The log of job `fail` only, ending with `this job fails on purpose` and the error line. `only_failed: false` adds the other job. |
| 16 | `pr_merge` `number: <the PR>`, `method: squash` | Merged, with a sha; `pr_read` says merged. |
| 17 | `release_create` `tag: v0.0.1`, `target: main`; `tag_check`; `tag_get`; `tag_get_all`; `release_read`; `release_list` | The tag is made by the release. Every read finds it. |
| 18 | `release_create` `tag: v0.0.1` again | Refused, saying the release exists. |
| 19 | `release_delete` `tag: v0.0.1`; `tag_check`; `tag_delete`; `tag_check` | The release is gone and the tag is still there; then the tag is gone. |
| 20 | `release_create` `tag: v0.0.2`, `draft: true`; `release_read` `tag: v0.0.2`; `release_delete` `tag: v0.0.2` | The draft is found by its tag and deleted. |
| 21 | `branch_create` `feat/x` and a file on it; `branch_merge` `base: main`, `head: feat/x`; the same merge again | A merge commit; the second says the branch is already up to date. |
| 22 | `branch_delete` for `feat/x` and `feat/smoke`; `branch_delete` `main` | Both deleted; `main` is refused. |
| 23 | `file_delete` `path: docs/a.md` | A commit; the file is gone from the tree and stays in history. |
| 24 | `repo_fork` `repo: octocat/Hello-World`; wait a moment; `repo_read` the fork | Accepted at once; readable shortly after. |
| 25 | `repo_delete` with a wrong `confirm`; then with the fork's full name | The first is refused and nothing is sent; the second deletes the fork. |
| 26 | `repo_delete` `repo: <you>/scratch-delete-me`, `confirm: <you>/scratch-delete-me` | The repository is gone. |

The workflow for row 13:

```yaml
name: Smoke
on:
  workflow_dispatch:
    inputs:
      word:
        description: A word to echo
        required: false
        default: hello
jobs:
  say:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ inputs.word }}"
  fail:
    runs-on: ubuntu-latest
    steps:
      - run: echo "this job fails on purpose" && exit 1
```

## Over HTTP

Run this once if you will use the HTTP transport. It needs only the server and a token; the walk above
then works unchanged through a client registered as in
[Set up HTTP with a token](setup-http-token.md).

| # | Do | Expect |
|---|---|---|
| H1 | `npm run start:http`, then `curl http://127.0.0.1:3001/healthz` | `{"status":"ok","server":"lxagents-github","version":"…"}` with no token. |
| H2 | `curl -i -X POST http://127.0.0.1:3001/mcp` | 401 with `WWW-Authenticate: Bearer realm="mcp"`. |
| H3 | `tools/list` with `Authorization: Bearer $GITHUB_TOKEN` (the curl in the guide) | The 45 tools. |
| H4 | `tools/call` `repo_read` on a repository your token can read, with that header | The repository, read as the token's owner. |
| H5 | The same call with a valid but unrelated GitHub token for another account | A 404 for a private repository the second account cannot see — each request acts as its own token. |
| H6 | The same call with a revoked or mistyped token | A message that GitHub rejected the token (401) and names the `Authorization: Bearer` header; the token is not in it. |
| H7 | Start the server with `MCP_AUTH_TOKEN` set and repeat H2 | A line on stderr saying it is ignored, and still a 401: the environment is never a fallback. |

## What the documentation leaves open

Each is a request shape the server sends on the strength of the documentation alone. If one behaves
differently, the tool named is the one to fix.

| Tool | What to look for |
|---|---|
| `branch_read`, `commit_compare` | A branch name with a slash is accepted as written, in a path and in `base...head`. |
| `branch_create` | `Accept: application/vnd.github.sha` on the commit lookup answers the bare sha. |
| `repo_fork` | 202, and the fork becomes readable shortly after. |
| `repo_search` | 30 requests a minute, and no more than the first 1000 matches. |
| `file_create`, `file_update` | An existing path is a 422 that names `sha`; a stale sha is a 409 or a 422. |
| `file_read` | A file over 1 MB is a 403 with `too_large`, and the refusal says what to do. |
| `commit_read` | The diff media type answers plain text; a very large commit answers 5xx, and the refusal says so. |
| `release_create` | A missing tag is created from `target`; a second release for the tag is a 422 `already_exists`. |
| `release_delete`, `release_read` | The tag survives the release; a draft is found only through the list. |
| `issue_open`, `issue_assign`, `issue_label` | With a token that has no push access, GitHub drops labels, assignees and the milestone without an error, and the tool names what it did not apply. |
| `issue_lock`, `issue_label` | A lock with no body is accepted; removing a label the issue does not have is a 404. |
| `pr_read` | `mergeable` is empty right after a push and settles on a later read. |
| `pr_merge` | A pull request with failing or pending required checks, or in a repository with a merge queue, is a 405; a stale `sha` is a 409. |
| `pr_approve`, `pr_review_request` | Approving your own pull request is a 422; so is a reviewer who is not a collaborator or is the author. |
| `pr_open` | A `head` of `owner:branch` opens a pull request from a fork; a second one for the same pair is a 422 saying it exists. |
| `workflow_run` | With `return_run_details: true` the pinned API version answers 200 with the run id, or 204 with none — either is handled. An input the workflow does not declare, or a workflow without a dispatch trigger, is a 422; a workflow file not on the default branch is a 404. |
| `workflow_status` | `per_page=1` on a workflow's runs returns the newest run; a branch name with a slash works in the `branch` filter. |
| `workflow_logs` | GitHub's 302 to a signed address is followed and the token is not sent there. A job still running gives its partial log or none; an expired log is noted; a log over 20 MB is refused with the general size message. |
| HTTP, any tool | The token in the `Authorization` header reaches `api.github.com` as the bearer credential of that request, and two interleaved requests with different tokens act as their own users. |
| the image | Build it, run `node src/index.js --version` in it, connect a client with `docker run --rm -i -e MCP_AUTH_TOKEN`, and check that `docker stop` returns at once. See [Docker](../environments/docker.md). |

## Reporting what differs

Open an issue on `LXAgents-MCP/github` with the tool, its arguments **without the token**, and the text
of the refusal or the result. A request shape that GitHub rejects is a defect in the tool and is fixed
in a release; the behaviours above that turn out otherwise are corrected in the next version's log, not
in this one.

## Related pages

* [Connect a client](connect-a-client.md)
* [Set up HTTP with a token](setup-http-token.md)
* [Environment](../environments/env.md) — what each tool needs from the token.
* [Security model](../security/security-model.md) — why nothing has been run live.
