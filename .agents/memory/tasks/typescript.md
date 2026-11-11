---
name: memory-tasks-typescript
description: Migrating the server and its tests from plain JavaScript to strict TypeScript compiled to dist/, with every dependency at its latest and a clean npm audit; done alongside gitlab, cli and template.
---

# TypeScript migration — GitHub

## 2026-11-11 — in progress

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
| 1 | Task record | `plan/typescript` | |
| 2 | Convert the server and its tests to TypeScript | `build/typescript` | |
| 3 | Corrected documentation | `docs/typescript` | |
| 4 | Release | `release/0.7.0` | |

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
