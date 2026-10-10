---
name: memory-tasks-forge-servers
description: Building this GitHub MCP server — moving create_github_repo out of cli and growing it to the 45-tool GitHub surface; released as 0.1.0.
---

# Forge servers — GitHub

## 2026-10-10 — complete

**Goal.** An LLM can work with GitHub — repositories, branches, files, commits, tags,
releases, issues, pull requests and Actions — through a dedicated MCP server, and the
host-shell server (`cli`) no longer carries any forge code.

**Objective.** `node src/index.js` serves 45 tools over stdio, named as the owner listed them.
`repo_create` is the `create_github_repo` tool moved out of `cli` 2.1.0, renamed, with the same
arguments and behaviour. The token is read from `LXAGENTS_MCP_GITHUB_API_KEY` at call time and
never appears in an argument, a log line or an error. `npm test` is green and no test reaches a
host beyond this machine. The repository has the structure of `shared-instruction`, without `content/`.

**Detail.** One request spans three repositories — `LXAgents-MCP/github`, `LXAgents-MCP/gitlab`
and `LXAgents-MCP/cli` — built in that order, each with its own record under this slug. `cli`
drops its two forge tools last, so `repo_create` exists elsewhere before it disappears there.
This repository consumes the shared instruction set through the connector and carries none of
it. Stdio only, Node 20 or newer, no build step; the runtime dependencies are the MCP SDK and
zod. GitHub means github.com. The moved code comes from `cli` commits `827b25a`, `c7d5bae` and
`5f9b64f`; its history is not transplanted.

**Status:** complete. Released as `0.1.0`; the pull requests, merged in order by rebase, are in the table.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/forge-servers-plan` | [#1](https://github.com/LXAgents-MCP/github/pull/1) |
| 2 | Server core and `repo_create` | `feat/server-core` | [#2](https://github.com/LXAgents-MCP/github/pull/2) |
| 3 | Agent instruction set | `docs/agents-setup` | [#3](https://github.com/LXAgents-MCP/github/pull/3) |
| 4 | Human documentation | `docs/wiki` | [#4](https://github.com/LXAgents-MCP/github/pull/4) |
| 5 | Repository tools | `feat/repo-tools` | [#5](https://github.com/LXAgents-MCP/github/pull/5) |
| 6 | Branch tools | `feat/branch-tools` | [#6](https://github.com/LXAgents-MCP/github/pull/6) |
| 7 | File and directory tools | `feat/file-tools` | [#7](https://github.com/LXAgents-MCP/github/pull/7) |
| 8 | Commit tools | `feat/commit-tools` | [#8](https://github.com/LXAgents-MCP/github/pull/8) |
| 9 | Tag and release tools | `feat/tag-release-tools` | [#9](https://github.com/LXAgents-MCP/github/pull/9) |
| 10 | Issue tools | `feat/issue-tools` | [#10](https://github.com/LXAgents-MCP/github/pull/10) |
| 11 | Pull request tools | `feat/pr-tools` | [#11](https://github.com/LXAgents-MCP/github/pull/11) |
| 12 | Workflow tools | `feat/workflow-tools` | [#12](https://github.com/LXAgents-MCP/github/pull/12) |
| 13 | Container image | `build/docker` | [#13](https://github.com/LXAgents-MCP/github/pull/13) |
| 14 | Release | `release/0.1.0` | [#14](https://github.com/LXAgents-MCP/github/pull/14) |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. A rebase merge rewrites commits, so
each branch was replayed onto the new `master` before its pull request was merged, in order, by rebase.

## Decisions the owner approved with the plan

Where a choice was needed, the owner's standing instruction is to offer the options with a
recommendation and use the recommended one. Each row names the option taken and the one declined.

| # | Decision |
|---|---|
| D1 | Branches follow `branching_strategy`: `chore/forge-servers-plan`, `feat/…`, `release/{version}`. The `claude/` prefix is not used, by the owner's instruction. Declined: one branch per repository. |
| D2 | Stdio only, as in `cli`. The server carries a token that can delete repositories, so an HTTP listener would let any holder of its bearer token act as the owner. Declined: stdio plus HTTP as in `shared-instruction`. HTTP would be its own approved task with a security record. |
| D3 | First release is `0.1.0`: a 45-tool surface whose write tools are exercised only against mocks. No git tag is created. |
| D4 | The package is `private: true`, as in `cli`. No npm publish. |
| D5 | Every tool except `repo_create` and `repo_search` takes one `repo` argument, `owner/name`, parsed and encoded in one place. Declined: separate `owner` and `repo` arguments. |
| D6 | The 45 tools are exactly the owner's list. Not added: `issue_list`, `pr_list`, `tag_create` (a tag comes from `release_create`), `issue_reopen`, inline review comments. |
| D7 | Tasks run strictly in order, with no checkpoint and no parallel agents. |
| D8 | Pull requests are opened one per branch and merged in order 1…n by rebase, each branch deleted as it merges — the owner gave both permissions. No tag and no npm publish. |
| D9 | Standing instruction: where a choice is needed, use the recommended option and add it to this table. |

## Out of scope

HTTP transport, GitHub Enterprise, CI workflows in this repository, live integration tests,
npm publishing, git tags.

## Task entries

### Task 1 — chore/forge-servers-plan

Landed: this record, `.agents/index/memory-index.md`, a minimal `.agents/index/root-index.md`
so the memory index has a parent, and `.gitignore`. The repository had no `.gitignore`; its
`/.agents/plans/` line keeps the untracked working plan out of history, and the owner approved
adding it with the plan. The root index lists only the memory index for now; task 3 completes
it. Nothing else changes in this task, and no code exists yet. The `PR` column is filled by task 14.

### Task 2 — feat/server-core

Landed. `node src/index.js` is a stdio MCP server named `lxagents-github` that serves one tool,
`repo_create`, the `create_github_repo` tool moved out of `cli` 2.1.0 with its arguments, defaults
and refusal wording unchanged. `package.json` is `private`, at `0.0.0` until the release task, with
the same two runtime dependencies as `cli`; the lockfile started from `cli`'s so the audited
dependency set is the same.

Shape. `src/api.js` is the only place a URL is built: `parseRepo`, `refName`/`refPath` and
`filePath` validate and percent-encode every path segment, so an argument cannot steer a call at
another endpoint. `github()` pins the API version, reads the token per call, and turns every
failure into one message that names the next step; a tool may word the statuses it knows better
through `explain`, which is how `repo_create` keeps its old messages. `callApi` is `cli`'s helper
with the response headers added, a raw mode for logs and diffs, and a cap on how much of an answer
it will read. `src/render.js` is one generic renderer for every tool: scalars become bullets, a
list of rows becomes a heading and bullets, and `text` becomes a fenced block. It replaces `cli`'s
per-shape branches, so the text of a `repo_create` result is now a bullet list and no longer
the two-line URL form. `src/tools/index.js` is `cli`'s registry unchanged.

Deviation from the plan, recorded: the argument and error helpers live in `src/` (`args.js`,
`errors.js`, `text.js`, `schemas.js`) and not in `src/tools/common.js`, so `src/tools/` holds only
tools and the registry test can hold files and tools together in both directions without an
exemption list.

Pinned. `test/surface.js` is the one place the tool list is written out, each tool tagged read,
write, update or destroy. The registry, protocol and stdio tests all compare against it, and the
registry test holds each tool's annotations to its kind. Adding a tool is therefore one file in
`src/tools/`, one line there, and its tests.

A crash is scrubbed like a refusal: the unexpected-exception path redacts the token from both the
log line and the text returned to the client, with a test that makes a handler throw the token.

Tests: the runner reports 93 passing and none failing. Two of those are the support files under
`test/`, which `node --test` also executes and counts. No test touches the network.

Left for later tasks: the other 44 tools (tasks 5-12), the instruction set and wiki (3, 4), the
image (13) and the release (14).

### Task 3 — docs/agents-setup

Landed. `AGENTS.md` is the entry point of a consuming repository: it declares the shared tools this
repository uses (`automation` and the four mandatory ones, plus `pull_request_template`), carries
the three gates and the discovery gate inline, says how to use the connector and what to say when it
does not resolve, and adds four trigger rows of its own. `.claude/CLAUDE.md` imports it so Claude
Code and every other agent read one file. The local set: two rules — `github-repository-rules` and
`tool-surface` — the agent wiki's `repository-map` and `security-boundaries` pages, a state file, the
stdio-only decision, and three indexes under a root index whose override table is empty.

Names. The local repository rule is `github-repository-rules`, not `repository-rules`, because the
shared set already has a file of that name and a local file with it would read as an undeclared
override. `shared-instruction`'s own local rule shares that name and its root index says no name
collides; that is raised as a finding for the owner and not touched.

Forward references. Four links in the agent wiki point at human wiki pages that task 4 writes
(`architecture`, `mcp-surface`, `env`, `security-model`). They resolve when task 4 lands; a link
check over `AGENTS.md` and `.agents/` finds no other unresolved link. The project wiki index and
the logs index are created with the pages they list, in tasks 4 and 14.

The security page holds the procedure: the isolation rule, a before-you-change table, what to
escalate and what is not a finding. Its surfaces tables moved to the human security model in task 4,
so facts live once.

Nothing here copies the shared set. Tests unchanged: 93 passing, none failing.

### Task 4 — docs/wiki

Landed. `README.md` is an overview that points at the wiki and states no tool count, and seven wiki
pages sit under a project wiki index: overview, architecture, MCP surface, setup, environment,
connect-a-client and the security model. `mcp-surface.md` and `env.md` carry the tables that grow:
each tool group adds its rows to both, and to the README and security model where it changes what
they say, in the same commit as the tools.

Facts once. The surfaces tables task 3 put in the agent security page now live in the human
security model; the agent page keeps the isolation rule, the before-you-change table, escalation
and what is not a finding. Claims about the `repo_delete` name-confirmation guard are left out of
the README and the security model until task 5 adds the tool, so no page describes code that does
not exist yet. The overview states scope, not inventory.

A link check over `README.md`, `wiki/`, `AGENTS.md` and `.agents/` finds no unresolved link. The
human pages say nothing about `.agents/`. Tests unchanged: 93 passing, none failing.

### Task 5 — feat/repo-tools

Landed. `repo_read`, `repo_update`, `repo_delete`, `repo_fork` and `repo_search`, pinned in
`test/surface.js` and documented in the surface page and the token table.

Checked against GitHub's REST documentation before writing: the update body parameters
(`archived: false` unarchives), delete's 204 and `delete_repo` scope, fork's three parameters and
its 202 with an asynchronous copy, and search's parameters, its 1000-result ceiling and its 30
requests a minute. `order` is ignored without `sort`, so the tool does not send it. GitHub's fork
page names no fine-grained permission, and the token table says so instead of guessing one.

Shape. `src/shapes.js` holds the repository schema and mapper that read, update and fork share, and
`expectBody`, which turns a 2xx answer of the wrong shape into a refusal. `maybeFlag` in
`src/args.js` keeps not-given apart from false, so an update sends only what changed. `REPO_NAME` is
exported from `src/api.js` and `repo_create` now shares it.

The guard. `repo_delete` refuses unless `confirm` repeats the full name, ignoring case, and a test
shows nothing is sent otherwise. The README and the security model carry that claim now that the
tool exists; task 4 left it out until then.

Tests: 112 passing, none failing.

### Task 6 — feat/branch-tools

Landed. `branch_create`, `branch_read`, `branch_list`, `branch_delete` and `branch_merge`, pinned in
`test/surface.js` and documented in the surface page and the token table.

Checked against GitHub's REST documentation: the `protected` filter on the list; the fully qualified
`refs/heads/` ref that create takes in its body; delete by `git/refs/heads/{branch}` answering 204,
with a 422 for the default branch; and merge's 201, 204, 409 and 404. The branch endpoint's page does
not say whether a branch name containing a slash works in the path. The tools keep slashes literal and
encode every other character, which is the form GitHub's git-refs pages use; confirming it is the
first thing a live smoke test should do.

`branch_create` makes two requests: the sha of the starting point, through GitHub's sha media type so
the answer is one line and not a whole commit with its file list, and then the ref. An empty
repository answers 409 to the first, and the refusal points at `file_create`. `branch_merge` is
annotated `destroy` because it moves the base branch, and a 204 is reported as `up_to_date`, not as a
failure.

Tests: 129 passing, none failing.

### Task 7 — feat/file-tools

Landed. `file_read`, `directory_list`, `file_create`, `file_update` and `file_delete`, pinned in
`test/surface.js` and documented in the surface page and the token table.

Checked against GitHub's REST documentation: the contents endpoint answers an object for a file and an
array for a directory; above 1 MB the default JSON answer is refused, which the tool turns into a
refusal that says to use git; a write takes the blob `sha` of the version it replaces, answers 201 for
a new file and 200 for an update, and a delete answers 200; and GitHub asks that writes to one
repository are made serially, which one-call-per-tool respects.

Shape. `src/contents.js` holds what the three write tools share: the API path, the base64 encoding
with a 1 MB ceiling, and `blobSha`, which reads the current sha when a caller did not pass one and
says what to do instead when there is no such file. `src/text.js` gained `decodeText`, `cli`'s UTF-8
then Windows-1252 decoder, `cutText`, which cuts in characters and never leaves half a surrogate
pair, and `looksBinary`. `file_read` cuts at 25 000 characters and says it did; it refuses binary
files, directories, symbolic links and submodules, each with the tool to use instead.

Corrected in a later commit on this branch: the limit was first written as 200 000 bytes, while a
result is rendered to at most 30 000 characters, so a 100 KB file reported `truncated: false` and was
cut anyway. The limit is now in characters and sits below that ceiling, so the flag is true to what
the model sees.

The security model now says plainly that everything these tools read is text someone else wrote.

Tests: 155 passing, none failing.

### Task 8 — feat/commit-tools

Landed. `commit_read`, `commit_list` and `commit_compare`, pinned in `test/surface.js` and documented in
the surface page and the token table.

Checked against GitHub's REST documentation: the list's filters (`sha`, `path`, `author`, `since`,
`until`); a commit's `stats`, `parents` and `files` (the files carry a `patch` that binary files lack);
the diff media type, which takes no paging and can time out on a very large commit; and compare's
`BASE...HEAD` form, its 250-commit and 300-file limits, and that commits come oldest first. Compare
shows the most recent 50 commits and the first 100 files and reports the totals; `commit_read` shows
the first 100 files. Cross-fork comparison (`owner:branch`) is not offered: a reference name may not
contain a colon.

The renderer changed: a string field with a line break, or too long for a bullet, is now a fenced
block under its own name, so a commit message keeps its lines instead of being squeezed onto one
bullet. It applies to every tool; the earlier groups' fields were all short and render as before.

`since` and `until` are checked as ISO 8601 before any request (`optionalDate`).

Tests: 170 passing, none failing.

### Task 9 — feat/tag-release-tools

Landed. `tag_check`, `tag_get`, `tag_get_all`, `tag_delete`, `release_create`, `release_read`,
`release_list` and `release_delete`, pinned in `test/surface.js` and documented in the surface page and
the token table.

Checked against GitHub's REST documentation: creating a release makes the tag from `target_commitish`
(the default branch if omitted) when it does not exist, and answers 201; getting a release by tag
returns only published releases, and the latest excludes drafts and prereleases; the list includes
drafts for a token with push access; deleting answers 204. The documentation does not say that deleting
a release keeps its tag, which is GitHub's long-standing behaviour; the tools say so, and it is on the
live-check list. An annotated tag is an object of its own read through `git/tags/{sha}`, and a
lightweight tag has none, so `tag_get` makes one request or two. `make_latest` exists on create and is
not offered.

Shape. `src/releases.js` holds `findReleaseByTag`: the direct lookup first, then up to three pages of
the list, because a draft cannot be fetched by tag and would otherwise be creatable but neither
readable nor deletable by the name it was created under. `releaseShape`, `releaseFields` and
`describeRelease` sit beside the repository ones in `src/shapes.js`. `tag_check` passes 404 to its
handler as an answer, which is what `allow` in `github()` is for.

`release_create` is how a tag is made, since the tool list has no `tag_create`; its description says so.

Tests: 196 passing, none failing.

### Task 10 — feat/issue-tools

Landed. `issue_open`, `issue_read`, `issue_comment`, `issue_close`, `issue_assign`, `issue_label` and
`issue_lock`, pinned in `test/surface.js` and documented in the surface page and the token table.

Checked against GitHub's REST documentation: creating an issue needs only a title, and **silently drops**
assignees, labels and a milestone for a token without push access; getting one carries a `pull_request`
key when the number is a pull request, and a `comments` count; an update takes `state` and
`state_reason`, and its `assignees` and `labels` replace the whole set; adding assignees answers 201 and
removing them 200 with a body on a DELETE; labels add (POST), replace (PUT) or remove one by name
(DELETE, 404 if the label does not exist); locking takes one of four reasons and answers 204, and a PUT
with no body is sent with a zero length. Because of the silent drops, `issue_open`, `issue_assign` and
`issue_label` return what GitHub applied and list what it did not (`not_applied`), instead of reporting
success for a request GitHub quietly shrank.

The renderer changed again: a long or multi-line field of a row in a list (a comment's body) is an
indented block under its bullet, so `issue_read` shows each comment in full instead of clipped to a
line. Short rows are unchanged.

`issue_close` posts its optional comment first, and if closing then fails it says the comment was
already posted. There is no reopen tool, as the owner's list has none.

Tests: 222 passing, none failing.

### Task 11 — feat/pr-tools

Landed. `pr_open`, `pr_read`, `pr_comment`, `pr_close`, `pr_merge`, `pr_review_request` and
`pr_approve`, pinned in `test/surface.js` and documented in the surface page and the token table.

Checked against GitHub's REST documentation: opening a pull request needs a title, a head and a base,
answers 201 and rejects with 422 when one is already open for the pair or the branches have no commit
between them; the head is `branch`, or `owner:branch` for a branch in a fork. Reading one returns
`mergeable` as `null` while GitHub works it out in the background, so `pr_read` leaves it out and says it
is still being computed rather than reporting "not mergeable"; the changed files come from their own
endpoint, at most 100 a page. A pull request's conversation comments are its issue's, so `pr_comment`
posts to `issues/{n}/comments`. Closing is an update of `state`. Merging is a PUT that takes
`merge_method`, an optional commit title and message and an optional `sha` the head must still match;
it answers 200, 405 when the pull request is not mergeable and 409 when the head moved, and both are
explained. Requesting reviewers answers 201 and 422 when a reviewer is not a collaborator or is the
author. An approval is a review with the event `APPROVE`, with an optional body; GitHub answers 422 when
the token's user opened the pull request.

Shape. `headRef` in `pr-open.js` allows `owner:branch` and applies the ordinary reference checks to the
branch half, so a colon only ever reaches the request body, never a URL. `reviewersOf` in
`src/shapes.js` joins a pull request's requested people and teams. `pr_close` posts its optional comment first and, as `issue_close`
does, says the comment was already posted if closing then fails. `pr_merge` is `destroy`; a body that
answers 2xx but not `merged: true` is returned as a refusal, not as a success.

Not offered, because the owner's list has no row for them: listing pull requests, reopening, inline
review comments, requesting changes, dismissing a review, enabling auto-merge, and updating a pull
request's branch.

Tests: 246 passing, none failing.

### Task 12 — feat/workflow-tools

Landed. `workflow_list`, `workflow_run`, `workflow_status` and `workflow_logs`, pinned in
`test/surface.js` and documented in the surface page, the token table and the security model.

Checked against GitHub's REST documentation: listing workflows answers `total_count` and `workflows`,
and every workflow endpoint takes the numeric id or the file name; runs are listed for a repository or
for one workflow and filter by `branch`, among others; a run's jobs are listed 100 to a page with a
`filter` that defaults to the latest attempt, each job carrying its `steps`; a job's log is a 302 to a
text file at an address that expires after a minute. The fine-grained permissions come from GitHub's
page of permissions required for fine-grained tokens: Actions set to read for everything here but
dispatch, and Actions set to write for dispatch.

Two things the documentation does not settle, and the tools work either way. First, the pages now
describe only API version 2026-03-10, where a dispatch takes `ref` and `inputs` and answers 200 with the
run's id and URLs; this server pins 2022-11-28, where a dispatch has answered 204 with no body.
`workflow_run` therefore asks for the run details with `return_run_details: true` — a parameter the
current page no longer lists, so it is unconfirmed — and accepts both answers: with an id it names the
run, without one it says to find the run with `workflow_status`. Second, the page does not state the
order of a workflow's runs; `workflow_status` takes the first of `per_page=1` as the latest, which is the
newest-first order GitHub has used, and that is on the live-check list.

Shape. `workflowName` in `src/api.js` is the fifth address helper: digits or one file name with no slash,
and a `.github/workflows/` path is reduced to its file name. `jobShape` and `jobFields` in `src/shapes.js`
are shared by status and logs; `failed_step` is the first step that failed, which is where to look in the
log. `workflow_run` writes every input as a string, as the workflow reads them, takes at most 25, and
looks up the default branch for `ref` the way `pr_open` does for `base`. `workflow_status` takes a run id
or a workflow with an optional branch, never both; a workflow with no run yet is `found: false`, an
answer a poller can act on, and not an error. `workflow_logs` reads the failed jobs by default, at most
five, one request each and one after another, sharing 20,000 characters so that each log keeps its
end; a job whose log is missing (404) or expired (410) gets a note and the rest are still read. `cleanLog`
in `src/text.js` removes the byte order mark and the colour codes and makes the line breaks one kind.

Two changes outside the new tools. `callApi` in raw mode now decodes an error answer as JSON, so a
refused log or diff carries GitHub's message and not the raw body text. And the log redirect goes to a
signed address on another host, which made "the token goes only to api.github.com" a claim to prove:
Node's `fetch` drops the `Authorization` header when a redirect changes host, checked here on Node 22
with two loopback servers, and a test in `test/api.test.js` holds `callApi` to it. The security model
and the agent security page say so, and the isolation of the token is unchanged.

Not offered, because the owner's list has no row for them: cancelling or re-running a run, enabling or
disabling a workflow, artifacts, secrets, a run's log archive, and the logs of an earlier attempt.

Tests: 277 passing, none failing.

### Task 13 — build/docker

Landed. A `Dockerfile` and `.dockerignore` for a stdio image, and `wiki/environments/docker.md`.

**The image has not been built.** The environment had a Docker client and no daemon, and starting one
was refused, so neither the file nor the commands on the page have run as written. What was checked
instead, and nothing more is claimed: every `COPY` source exists and none is excluded by
`.dockerignore`; the install step, `npm ci --ignore-scripts --omit=dev`, was run in a scratch directory
holding only the files the image copies, and the production tree it made (the MCP SDK and zod) served
all 45 tools over a real stdio pipe and refused a call with no token; and no `ENV`, `EXPOSE` or
`ENTRYPOINT` line exists. Not checked: the base image, the file's syntax, and `USER node` in a real
container. It is on the live-check list to build once and run `node src/index.js --version` in it.

Shape. One stage on `node:22-alpine`, because there is no build step; dependencies from the lockfile
with scripts off; `USER node`; `CMD ["node", "src/index.js"]`. Unlike the image of the shared set it
declares no port and sets no `ENTRYPOINT`: there is no listener to declare, and an entrypoint of ours
would make a command after the image name run as an argument of it. The token is passed with
`docker run -e LXAGENTS_MCP_GITHUB_API_KEY`, never an `ENV` line. The page tells a client to use
`-i` and never `-t`, and ties the tag to the version in `package.json` without naming one.

One change to the server came with it. `src/index.js` now closes and exits on `SIGTERM` as well as
`SIGINT`. In a container the server is the first process, which the kernel gives no default action for
a signal, so `docker stop` would otherwise wait out its grace period and kill it. The signal handlers
are installed before the "serving" line is logged, and `test/stdio.test.js` sends each signal to a real
process; without the handler the `SIGTERM` test fails, which was confirmed.

The docs that state the process contract, the repository map, the security procedure and the
deployment rule now cover the image, and the state file no longer says there is none.

Tests: 279 passing, none failing.

### Task 14 — release/0.1.0

Landed. The version is `0.1.0` in `package.json` and the lockfile — the owner approved it with the plan
(D3) — and no git tag was made and nothing was published.

The release log is `wiki/logs/0/1/0/CHANGELOG.md`, in the house shape: a lead, the **Clients must**
steps, then Added, Changed and Security. It says first that nothing has run against live GitHub. Its
index, `.agents/index/logs-index.md`, is new and routed from the root index; the state file is
rewritten to describe a released repository.

The live-check list is not in the release log but in `wiki/guides/smoke-test.md`: a walk through every
tool on a throwaway repository, then each behaviour the documentation leaves open with what to look for.
A released log may not be edited, and the owner will work through this list and learn which items hold;
a page can follow what they find, and the next release's log carries the corrections.

Checks run on the release branch from a clean install (`rm -rf node_modules && npm ci`): 279 tests pass
and none fail; the production tree is the MCP SDK and zod; nothing under `src/` imports `node:http`,
`node:net`, `node:cluster` or `express`; the `Dockerfile` has no `ENV`, `EXPOSE` or `ENTRYPOINT` line;
`node src/index.js --version` prints `lxagents-github 0.1.0`.

Because the redirect test uses the real `fetch` against two servers on loopback, the sentences that said
the tests touch no network now say they reach no host beyond this machine: the repository rules, the
README, the setup page and this record's objective.

Left for the owner: the smoke test, building the image once, and whether and where to publish either.
The `PR` column of the table above was filled by a closing commit once the pull requests existed.

### Closing

Landed. All 14 pull requests were merged in order, #1 to #14, each by rebase, and `master` is a linear
history of 16 commits after the initial one. The tree of `master` is identical to the tip of
`release/0.1.0` as it stood before the first rebase, and from a clean install 279 tests pass and
`node src/index.js --version` prints `lxagents-github 0.1.0`.

The merged branches were not deleted as planned: the git proxy refused to delete a remote branch (HTTP
403) and was not retried. The 14 branches named in the table are still on the remote and can be deleted
by the owner; none of them is needed.
