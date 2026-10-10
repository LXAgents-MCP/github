# MCP Surface

Everything `lxagents-github` exposes.

**It exposes tools, and nothing else.** One per module in `src/tools/`. There are no prompts and no
resources, and one transport: stdio. See [What this server does not expose](#what-this-server-does-not-expose).

## Server identity

| Field | Value |
|---|---|
| `name` | `lxagents-github` |
| `version` | from `package.json`, read at import by `src/version.js` |
| transport | stdio (`src/index.js`) |

`initialize` also returns `instructions`: that every tool needs `LXAGENTS_MCP_GITHUB_API_KEY`, that
a repository is named `owner/name`, and that tools which delete or merge are destructive and should
be confirmed with the owner first.

## Conventions every tool shares

### Naming

A tool is `{thing}_{action}`: `repo_create`, `branch_list`, `pr_merge`. The module that defines it
is the same name in kebab case, `src/tools/pr-merge.js`.

### Addressing a repository

Every tool that acts on a repository takes one `repo` argument, written `owner/name`. The two that
cannot are `repo_create`, which has no repository yet, and `repo_search`, which looks across all of
them. `repo` is parsed once, in `src/api.js`, and a value that is not exactly two valid names is
refused before any request.

### Results

Each result is readable text plus `structuredContent` that matches the tool's output schema. The
text opens with a one-sentence summary, then lists the fields; long content — a file, a log, a diff
— is a fenced block cut at a stated limit, with `truncated: true` when it was cut.

### Paging

A list takes `page` (from 1) and `per_page` (1 to 100, default 30) and returns `page`, `per_page`
and `has_more`. Ask for the next page while `has_more` is true.

### Annotations

| Kind | Meaning | Hints |
|---|---|---|
| `read` | Only reads | read-only, idempotent |
| `write` | Creates something | none of those |
| `update` | Changes something that exists | idempotent |
| `destroy` | Deletes, overwrites or merges | destructive, idempotent |

All four set `openWorldHint`, because every tool talks to GitHub. `test/surface.js` pins every
tool's kind, and a test holds the annotations to it.

### Failures

A failure is an `isError` result whose text says what happened and what to do: a rejected token
(401), a missing permission (403), a rate limit with its reset time, a repository the token cannot
see (404), a conflict (409), a validation error with GitHub's own reasons (422). The token never
appears in one.

## The tools

### Repositories

| Tool | Kind | Does |
|---|---|---|
| `repo_create` | write | Create a repository, in the token owner's account or an organization. Private unless told otherwise, with an initial README. Moved from `cli`, where it was `create_github_repo`. |
| `repo_read` | read | Read one repository: description, visibility, default branch, star, fork and issue counts, language, license, topics and parent. |
| `repo_update` | update | Change settings: rename, description, homepage, private or public, default branch, archive or unarchive, the issues switch. Only what is passed changes; at least one is required. |
| `repo_delete` | destroy | Permanently delete a repository. `confirm` must repeat its full name, and anything else deletes nothing. |
| `repo_fork` | write | Fork into the token owner's account or an organization. GitHub copies it in the background, so the fork may take a moment to be readable. |
| `repo_search` | read | Search repositories with GitHub's search syntax. Paged; GitHub serves the first 1000 matches and limits search to 30 requests a minute. |

### Branches

| Tool | Kind | Does |
|---|---|---|
| `branch_create` | write | Create a branch from the default branch, another branch, a tag or a sha. Fails if the name is taken. |
| `branch_read` | read | Read one branch: the commit at its tip, that commit's message and date, and whether it is protected. |
| `branch_list` | read | List branches with the sha at each tip. Paged; can filter to protected or unprotected. |
| `branch_delete` | destroy | Delete a branch. The default branch cannot be deleted. |
| `branch_merge` | destroy | Merge a branch or commit into another branch directly, with no pull request. Moves the base branch. |

### Files and directories

Changes are made by committing through the API, one commit per call, on a branch you name (default:
the default branch). `file_update` and `file_delete` read the file's current sha first unless you pass
one, so a change fails if the file moved on in between.

| Tool | Kind | Does |
|---|---|---|
| `file_read` | read | Read one text file at a branch, tag or commit. Cut at 25,000 characters; binary files and files over 1 MB are refused. Returns its content, size and blob sha. |
| `directory_list` | read | List the files and folders in one directory, not recursively. Omit the path for the root. |
| `file_create` | write | Create a new file as one commit. Fails if it exists. Content up to 1 MB. |
| `file_update` | update | Replace an existing file's whole content as one commit. Fails if it changed since the sha. |
| `file_delete` | destroy | Delete one file as one commit. The file stays in history. |

### Commits

| Tool | Kind | Does |
|---|---|---|
| `commit_read` | read | Read one commit: message, author, date, parents, and the files it changed with line counts. `diff: true` adds its diff, cut at 20,000 characters. Defaults to the default branch's tip. |
| `commit_list` | read | List commits, newest first, from a branch, tag or sha; optionally only those touching a path, by an author, or within a date range. Paged. |
| `commit_compare` | read | Compare two refs: ahead and behind, the most recent 50 commits between them, and the changed files (the first 100) with line counts. |

### Tags and releases

A tag is created by `release_create`, which makes the tag from a branch or commit when it does not
exist; there is no separate tag-creating tool.

| Tool | Kind | Does |
|---|---|---|
| `tag_check` | read | Say whether a tag exists, and if so what it points at and whether it is annotated. A missing tag is an answer, not an error. |
| `tag_get` | read | Read one tag: the commit behind it and, for an annotated tag, its message, tagger and date. |
| `tag_get_all` | read | List tags with the commit each points at. Paged. |
| `tag_delete` | destroy | Delete a tag. It does not delete a release. |
| `release_create` | write | Create a release for a tag, as a draft or prerelease if asked, with GitHub-written notes if asked. |
| `release_read` | read | Read one release by tag, or the latest published one. Finds a draft by tag too, through the list. |
| `release_list` | read | List releases, newest first, including drafts for a token with push access. Paged. |
| `release_delete` | destroy | Delete a release by tag or by id. Its tag is kept. |

### Issues

A pull request is an issue to GitHub, so these tools also work on a pull request's number. GitHub
**silently ignores** labels, assignees and a milestone from a token without push access, and people who
cannot be assigned, so `issue_open`, `issue_assign` and `issue_label` report what was actually applied
and name anything that was not.

| Tool | Kind | Does |
|---|---|---|
| `issue_open` | write | Open an issue with an optional body, labels, assignees and milestone. |
| `issue_read` | read | Read one issue: state, author, labels, assignees, milestone and body; `comments: true` adds the first 30 comments. Says when the number is a pull request. |
| `issue_comment` | write | Add a comment. Says when the conversation is locked. |
| `issue_close` | update | Close as completed, not planned or a duplicate, optionally commenting first. There is no tool to reopen. |
| `issue_assign` | update | Add (default), remove or replace assignees. |
| `issue_label` | update | Add (default), remove or replace labels. |
| `issue_lock` | update | Lock, with a reason, or unlock. |

### Pull requests

A pull request's conversation is its issue's, so `pr_comment` writes the same comments `issue_read`
shows. `pr_merge` is the one tool here that moves a branch: it is `destroy`, and the server's
instructions tell a client to confirm it with the owner first.

| Tool | Kind | Does |
|---|---|---|
| `pr_open` | write | Open a pull request from a branch into a base branch (default: the default branch), as a draft if asked. A branch in a fork is `owner:branch`. Says when one is already open or the branches do not differ. |
| `pr_read` | read | Read one pull request: state, draft, merged, whether it can be merged, branches, size, labels, requested reviewers and description; `files: true` adds the first 100 changed files without their diffs. Says so when GitHub has not yet worked out mergeability, rather than answering no. |
| `pr_comment` | write | Add a comment to the conversation. It is not attached to a line of the diff and is not a review. Says when the conversation is locked. |
| `pr_close` | update | Close without merging, optionally commenting first. The branch is kept. There is no tool to reopen. |
| `pr_merge` | destroy | Merge by merge commit (default), squash or rebase. Pass `sha` to merge only if the branch is still at the commit that was reviewed. Says why when GitHub refuses (conflicts, checks, approvals, a disabled method, a merge queue). |
| `pr_review_request` | update | Ask people, teams or both to review. They must be collaborators and cannot be the author. |
| `pr_approve` | write | Approve as the token's user, with an optional message. A person cannot approve their own pull request. |

### Workflows

GitHub Actions. A workflow is named by its id or by the name of its file under `.github/workflows/`,
such as `ci.yml`; `workflow_list` shows both. A run is named by its id. GitHub serves a job's log from
a signed address on another host, and the request follows the redirect without the token: see
[Security model](../security/security-model.md).

| Tool | Kind | Does |
|---|---|---|
| `workflow_list` | read | List a repository's workflows with their id, name, file path and state. Paged. |
| `workflow_run` | write | Start a workflow by dispatch on a branch or tag (default: the default branch), with optional inputs. It runs whatever the workflow does, which may deploy or publish. The workflow needs an `on: workflow_dispatch` trigger and its file must be on the default branch. Returns the run id when GitHub gives it, and otherwise says to find the run with `workflow_status`. |
| `workflow_status` | read | Read one run — status, conclusion, branch, commit — and its jobs, with the step that failed in each. Give a `run_id`, or a workflow (optionally on a branch) to read its latest run. A workflow with no run yet is an answer, not an error. |
| `workflow_logs` | read | Read the logs of a run's jobs: the failed ones by default (up to five), one with `job_id`, or every job with `only_failed: false`. At most 20,000 characters in all, shared between the jobs, and each log cut to its end, where a failure is explained. Colour codes and the byte order mark are removed. A job with no log, or an expired one, is noted and the others are still read. |

## What this server does not expose

| Not exposed | Why |
|---|---|
| Prompts and resources | Everything an agent needs is a call with arguments. |
| Local files and commands | That is `cli`'s job; this server touches GitHub only. |
| HTTP | The token can delete repositories. See [Security model](../security/security-model.md). |
| GitHub Enterprise | The API base is a constant, and another host would need a second setting. |
| `clone`, `push` and other git transport | File tools commit through the API instead. |
| `issue_list`, `pr_list`, `tag_create`, `issue_reopen`, inline review comments, cancelling or re-running a run, enabling or disabling a workflow, artifacts, secrets | Outside the tool list this server was built to. A tag is created by `release_create`. |
