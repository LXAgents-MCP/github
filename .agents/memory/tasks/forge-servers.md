---
name: memory-tasks-forge-servers
description: Building this GitHub MCP server — moving create_github_repo out of cli and growing it to the 45-tool GitHub surface; released as 0.1.0.
---

# Forge servers — GitHub

## 2026-10-10 — in progress

**Goal.** An LLM can work with GitHub — repositories, branches, files, commits, tags,
releases, issues, pull requests and Actions — through a dedicated MCP server, and the
host-shell server (`cli`) no longer carries any forge code.

**Objective.** `node src/index.js` serves 45 tools over stdio, named as the owner listed them.
`repo_create` is the `create_github_repo` tool moved out of `cli` 2.1.0, renamed, with the same
arguments and behaviour. The token is read from `LXAGENTS_MCP_GITHUB_API_KEY` at call time and
never appears in an argument, a log line or an error. `npm test` is green and no test touches
the network. The repository has the structure of `shared-instruction`, without `content/`.

**Detail.** One request spans three repositories — `LXAgents-MCP/github`, `LXAgents-MCP/gitlab`
and `LXAgents-MCP/cli` — built in that order, each with its own record under this slug. `cli`
drops its two forge tools last, so `repo_create` exists elsewhere before it disappears there.
This repository consumes the shared instruction set through the connector and carries none of
it. Stdio only, Node 20 or newer, no build step; the runtime dependencies are the MCP SDK and
zod. GitHub means github.com. The moved code comes from `cli` commits `827b25a`, `c7d5bae` and
`5f9b64f`; its history is not transplanted.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/forge-servers-plan` | |
| 2 | Server core and `repo_create` | `feat/server-core` | |
| 3 | Agent instruction set | `docs/agents-setup` | |
| 4 | Human documentation | `docs/wiki` | |
| 5 | Repository tools | `feat/repo-tools` | |
| 6 | Branch tools | `feat/branch-tools` | |
| 7 | File and directory tools | `feat/file-tools` | |
| 8 | Commit tools | `feat/commit-tools` | |
| 9 | Tag and release tools | `feat/tag-release-tools` | |
| 10 | Issue tools | `feat/issue-tools` | |
| 11 | Pull request tools | `feat/pr-tools` | |
| 12 | Workflow tools | `feat/workflow-tools` | |
| 13 | Container image | `build/docker` | |
| 14 | Release | `release/0.1.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by
task 14, once the pull requests exist.

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
