# Overview

`LXAgents-MCP/github` is an MCP server, `lxagents-github`, that lets an LLM work with GitHub
through its REST API: read and change repositories, branches, files, commits, tags, releases,
issues, pull requests and Actions workflows. It speaks stdio, and the client starts it as a
subprocess.

## Why it is its own server

GitHub access first lived in the host-shell server, `cli`: one tool, `create_github_repo`, beside
`run_command`, `read_file` and `list_directory`. That put a credentialed API client in the same
process as a tool that runs arbitrary commands, and `cli`'s own documentation had to explain that
its `--confine` and `--allow` flags bound the shell tools and do not apply to the GitHub one. Two
trust boundaries shared one process.

Split, each server has one job and one boundary. `cli` runs commands and reads files on the host
and makes no network call of its own. This server calls `api.github.com` (and, to read a job's log, the
signed address GitHub redirects to), reads no local file and runs no command. `repo_create` moved here from `cli`, renamed from `create_github_repo`, with its
arguments and behaviour unchanged. GitLab is the same arrangement in its own repository.

## What it serves

**Tools, and nothing else.** No prompts and no resources. The server covers the families of
operation an agent needs on a repository, each with its own tool prefix:

| Prefix | Acts on |
|---|---|
| `repo_` | Repositories: create, read, update, delete, fork, search. |
| `branch_` | Branches: create, read, list, delete, merge. |
| `file_`, `directory_list` | Files and directories, changed by committing through the API. |
| `commit_` | History: read, list, compare. |
| `tag_`, `release_` | Tags and releases. |
| `issue_` | Issues: open, read, comment, close, assign, label, lock. |
| `pr_` | Pull requests: open, read, comment, close, merge, request a review, approve. |
| `workflow_` | Actions workflows: list, run, status, logs. |

Which of these exist in a given version is listed, tool by tool, in
[MCP surface](../reference/mcp-surface.md).

## Why the token is in the environment

A tool that took the token as an argument would put it in a prompt and a transcript. This server
reads `LXAGENTS_MCP_GITHUB_API_KEY` from its own environment, set by the client's configuration,
and reads it when a tool is called rather than at startup, so a server without one still starts
and says what is missing when asked. See [Environment](../environments/env.md).

## What it is not

* **Not a git client.** It does not clone or push. File tools commit through the API.
* **Not GitHub Enterprise.** The API base is a constant; supporting another host would need a
  second setting, which is a decision for the owner.
* **Not a service.** There is no HTTP transport. See [Security model](../security/security-model.md)
  for why.

## Related pages

* [Architecture](architecture.md) — how a call becomes a request and a result.
* [MCP surface](../reference/mcp-surface.md) — every tool.
* [Security model](../security/security-model.md)
