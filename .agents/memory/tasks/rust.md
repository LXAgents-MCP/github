---
name: memory-tasks-rust
description: Rewriting the server and its tests from TypeScript in Rust, on the official MCP Rust SDK (rmcp) with axum for HTTP, with the tool surface unchanged; done alongside gitlab, cli and template.
---

# Rust rewrite — GitHub

## 2026-11-14 — planned

**Goal.** The owner asked for every repository to move from TypeScript to Rust, using the official MCP
Rust SDK (`rmcp`, <https://github.com/modelcontextprotocol/rust-sdk>) and `axum` as the web framework.

**Objective.**

* The server is one Rust package at the repository root: a library with the six layers
  (`routes`, `controllers`, `services`, `models`, `utils`, `config`) and the `lxagents-github` binary.
* `rmcp` serves the protocol over stdio and, inside an axum app, over streamable HTTP; `reqwest` with
  rustls makes the calls to GitHub.
* The 45 tools are unchanged: names, descriptions, input and output schemas and annotations match the
  0.7.0 `tools/list`, which is kept as a golden fixture and compared by a test.
* Every request, summary, field, refusal and rendered text is unchanged, as the ported tests show.
* stdio and HTTP keep every rule they had: the two secrets, the access token, the local-install `Host`
  and `Origin` checks, the routes, statuses, start-up refusals, log lines and exit codes.
* `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` are clean; the
  TypeScript sources, `package.json`, the lockfile and the tsconfig files are gone.

**Detail.** A rewrite of the implementation, not of the product: no tool, argument, description,
variable, flag, route, status or message changes. The plan in full is
`/mnt/project-files/reports/rust-github-plan.md` in the project's shared files; it follows the
`gitlab` plan section for section. The owner is away and asked for the recommended option wherever a
question comes up. Baseline: 315 passing on Node 22, `master` at `22b57d9`.

The shared instruction connector was not reachable in this session, so `commit_conventions` and the
other shared tools could not be read. Commits follow this repository's own history: Conventional
Commits, a bulleted body, and the one `Co-Authored-By` trailer.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/rust` | |
| 2 | Rewrite the server and its tests in Rust | `refactor/rust` | |
| 3 | Corrected documentation | `docs/rust` | |
| 4 | Release | `release/0.8.0` | |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. `gitlab`, `cli` and `template` run
their own chains; no chain depends on another.

## Decisions

| # | Decision |
|---|---|
| R1 | The release is `0.8.0`: the tools do not change, but a client now runs the `lxagents-github` binary instead of `node dist/index.js`, which is breaking under 0.x. No tag, nothing published. |
| R2 | One package, `lib` plus `bin`, at the repository root. Tests stay in `test/`, as one integration-test binary declared in `Cargo.toml`. |
| R3 | Everything a handler reads from outside — the environment and the network — comes through a context value, so parallel Rust tests can mock both without touching process state. |
| R4 | Schemas are JSON written in each tool's module and pinned by the golden 0.7.0 `tools/list`. The one key `rmcp` cannot carry, `execution.taskSupport: "forbidden"`, is what an absent key means, and the comparison ignores it. |
| R5 | String lengths and cuts count Unicode scalar values, not UTF-16 code units; the two differ only beyond the Basic Multilingual Plane, and no cut can split a character. |
| R6 | The owner's request is the agreement `.agents/rules/repository.md` asks for before the stack changes. The factual stack lines in the rule files are corrected in the docs task; any new rule is proposed, not written. |
| R7 | The runtime dependency set grows past the SDK and its schema library, recorded before the code lands in `decisions/rust-runtime-dependencies.md`. |

## Task entries

### Task 1 — plan/rust

Landed: this record, `decisions/rust-runtime-dependencies.md`, and their rows in `memory-index.md`.
Nothing outside `.agents/` changes. The `PR` column is filled by the release task.
