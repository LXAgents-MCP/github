---
name: memory-tasks-typescript
description: Migrating the server and its tests from plain JavaScript to strict TypeScript compiled to dist/, with every dependency at its latest and a clean npm audit; done alongside gitlab, cli and template.
---

# TypeScript migration — GitHub

## 2026-11-11 — complete

**Goal.** The owner asked for every Node.js repository to move to TypeScript, so the code carries its
types instead of JSDoc comments that nothing checks.

**Objective.**

* Every dependency at its latest version, `typescript`, `@types/node` and `tsx` added as dev
  dependencies, and `npm audit` reporting no vulnerabilities.
* A `tsconfig.json` for Node ESM: `target` `ES2022`, `module` and `moduleResolution` `NodeNext`,
  `rootDir` `./src`, `outDir` `./dist`, `strict`.
* Every `.js` file in `src/` and `test/` renamed to `.ts` and typed, with no implicit `any`.
* `package.json`: `main` and `bin` point at `dist/index.js`, `build` is `tsc`, the start scripts run
  `dist/`, and `test` is `node --import tsx --test test/*.test.ts`.
* `npm run build` compiles with no error, `npm test` passes, and the stdio and HTTP transports and the
  tool surface behave exactly as before.

**Detail.** A conversion, not a rewrite: no tool, argument, description, variable, route, status or
message changes. Declarations keep their order and their comments; JSDoc type tags give way to
TypeScript types. `dist/` is build output and is ignored, never committed. The owner is away and asked
for the recommended option wherever a question comes up. Baseline: 317 passing on Node 22.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/typescript` | [#42](https://github.com/LXAgents-MCP/github/pull/42) |
| 2 | Convert the server and its tests to TypeScript | `build/typescript` | [#43](https://github.com/LXAgents-MCP/github/pull/43) |
| 3 | Corrected documentation | `docs/typescript` | [#44](https://github.com/LXAgents-MCP/github/pull/44) |
| 4 | Release | `release/0.7.0` | [#45](https://github.com/LXAgents-MCP/github/pull/45) |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. `gitlab`, `cli` and `template` run
their own chains; no chain depends on another.

## Decisions

| # | Decision |
|---|---|
| D1 | The owner's request is the agreement `.agents/rules/repository.md` asks for before a build step or TypeScript is introduced. Editing that rule is proposed to the owner, not done here. |
| D2 | Tests run the TypeScript sources through `tsx`, including the two that spawn the server, so a test never depends on a stale `dist/`. |
| D3 | The tool registry discovers `.ts` modules when run from source and `.js` modules when run from `dist/`; declaration files are skipped. |
| D4 | The image gains a build stage: the runtime stage holds production dependencies and `dist/` only. |
| D5 | The release is `0.7.0`: a client that runs `node src/index.js` must now build and run `node dist/index.js`, which is breaking under 0.x. No tag, nothing published. |

## Task entries

### Task 1 — plan/typescript

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes. The `PR`
column is filled by the release task.

### Task 2 — build/typescript

Landed.

* **Dependencies.** `@modelcontextprotocol/sdk` 1.32.2 and `zod` 4.6.7, the latest; `typescript` 7.0.2,
  `@types/node` 26.6.6 and `tsx` 4.23.15 as dev dependencies. No Express import, so no
  `@types/express`. `npm audit fix` cleared four advisories in transitive dependencies (`body-parser`,
  `fast-uri`, `ip-address`, `qs`); `npm audit` reports 0.
* **Configuration.** `tsconfig.json` builds `src/` to `dist/` (ES2022, NodeNext, strict, `types: ["node"]`,
  `skipLibCheck`). `tsconfig.test.json` extends it with no emit over `src/` and `test/`, run by
  `npm run typecheck`, so the tests are held to the same strict rules without being compiled.
  `dist/` is ignored by git and by the image context.
* **Conversion.** Every file renamed `.js` to `.ts`; relative imports keep their `.js` specifiers, which
  NodeNext resolves to the `.ts` source. JSDoc types became TypeScript types and the type tags were
  dropped from the comments; the prose stays. New type-only modules: `src/models/tool.ts` (`ToolDefinition`,
  `ToolResult`, `Payload`, `ToolSpec`) and `src/models/github-object.ts` (`GitHubObject`, a JSON object as
  GitHub returns it, read field by field). `ApiAnswer` and `CallOptions` in `src/services/client.ts`,
  `GitHubOptions` in `src/services/api.ts`, `Explain` and `FailureContext` in `src/services/failure.ts`,
  `HttpServerOptions` and `RunningHttpServer` in `src/routes/http.ts`, `ErrorLike` in `src/utils/errors.ts`.
  Overloads on `oneOf`, `rawText` and `textList` give a non-optional result when a fallback or
  `required: true` is passed, replacing three JSDoc casts. The only explicit `any`s are the decoded body of
  an answer and `GitHubObject`, both documented.
* **Registry.** `isToolModule` accepts `.ts` and `.js` modules, never `.d.ts` or the registry itself, and the
  registry test uses it, so the file-name check holds from `src/` and from `dist/`.
* **Behaviour.** One visible text changes on purpose: `--help` says `Usage: node dist/index.js`. The
  `tools/list` answer of the built server is byte-identical to the one `master` serves.
* **package.json.** `main` and `bin` are `dist/index.js` and `files` ships `dist`; `build` is `tsc`;
  `start`, `start:http` and `inspect` run `dist/`; `test` is `node --import tsx --test test/*.test.ts`.
* **Image.** The Dockerfile has a build stage that runs `npm ci` and `npm run build`, and a runtime stage
  with `npm ci --omit=dev` and `dist/` only; `CMD` is `node dist/index.js`. There is no Docker daemon here,
  so the image was not built; its steps were replayed by hand in a scratch directory and the result
  printed its version.

Checked: `npm run build` and `npm run typecheck` pass with no error; `npm test` passes 315 of 315. The
total drops from 317 only because the runner no longer executes `test/helpers.ts` and `test/surface.ts`,
which define no tests. The built server answered `--version`, `--help`, `tools/list` over stdio, and
`/healthz` and `tools/list` over HTTP from both `dist/index.js` and `dist/routes/http.js`.

### Task 3 — docs/typescript

Landed. Every page that named a source file says `.ts`; every command that runs the server says
`dist/index.js` and is preceded by `npm run build` where a reader starts from a checkout: `README.md`,
`wiki/environments/{setup,env,docker}.md`, `wiki/guides/{connect-a-client,setup-http-token,smoke-test}.md`,
`wiki/reference/mcp-surface.md`, `wiki/security/security-model.md`, `wiki/information/architecture.md` (a new
*Source and build* section and the two new model files), `.agents/wiki/context/repository-map.md` (the
commands, the `tsconfig` and `dist/` rows, and three gotchas: a stale `dist/`, `.js` import specifiers, the
new test total), `.agents/wiki/security/security-boundaries.md`, and `memory/state/repository-state.md`.
Release logs and older task records keep the old paths on purpose.

Left stale on purpose, and proposed to the owner instead: `.agents/rules/repository.md` still says there is no
build step and no TypeScript and that the code is plain JavaScript, and lists `.js` paths;
`.agents/rules/tool-surface.md` still names `repo-create.js` and the `.js` support files. They are instruction
files, which the discovery protocol does not let this task edit.

### Task 4 — release/0.7.0

Landed: `package.json` and the lockfile's own version are `0.7.0`; `wiki/logs/0/7/0/CHANGELOG.md` and its row in
`logs-index.md`, newest first; `memory/state/repository-state.md` names the version. No tag, nothing
published. The `PR` column was filled by a closing commit on this branch once every pull request was open, and the record marked complete.
