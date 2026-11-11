# 0.7.0

**Released:** 2026-11-11

**The server is TypeScript, and what runs is the compiled `dist/`.** Every file in `src/` and `test/` is
TypeScript under `strict`, and `npm run build` compiles `src/` to `dist/`. The 45 tools, their arguments,
results and messages, the tokens, the variables and the routes are as they were: the `tools/list` answer
of the built server is byte-identical to 0.6.1's. **Breaking for one path:** `src/index.js` and
`src/routes/http.js` no longer exist; a client runs `dist/index.js`, which a checkout has only after a build.

**Clients must:** run `npm install` and `npm run build` in the checkout, then change the server's entry
from `.../src/index.js` to `.../dist/index.js` (and `node src/routes/http.js` to `node dist/routes/http.js`,
or use `npm run start:http`). Build again after every pull. Rebuild the image. Restart the client.

## Added

* `tsconfig.json` (ES2022, NodeNext, `strict`, `src/` to `dist/`) and `tsconfig.test.json`, which checks
  `src/` and `test/` together with no output.
* Scripts: `build` (`tsc`) and `typecheck`.
* Dev dependencies: `typescript` 7.0.2, `@types/node` 26.6.6, `tsx` 4.23.15.
* `src/models/tool.ts`, the contract a tool module meets, and `src/models/github-object.ts`, a JSON object
  as GitHub returns it. Both are types only.
* A build stage in the `Dockerfile`; the image ships `dist/` and production dependencies only.

## Changed

* Every `.js` file in `src/` and `test/` is now `.ts`.
* `main`, `bin`, `start`, `start:http` and `inspect` point at `dist/`; `files` ships `dist`.
* `npm test` is `node --import tsx --test test/*.test.ts`, run from source with no build. It reports 315
  tests: the two support files, which define none, are no longer run on their own.
* `--help` says `Usage: node dist/index.js`.
* The registry discovers `.ts` modules from `src/` and `.js` modules from `dist/`.
* `@modelcontextprotocol/sdk` 1.32.2, `zod` 4.6.7.

## Security

* `npm audit fix` cleared one high and three moderate advisories in transitive dependencies: `body-parser`,
  `fast-uri`, `ip-address` and `qs`. `npm audit` reports 0 vulnerabilities.

## Verified

`npm run build` and `npm run typecheck` pass with no error; `npm test`: 315 passing, none failing. The built
server answered `--version`, `--help` and `tools/list` over stdio and over HTTP. The image's steps were
replayed by hand; the image itself has still never been built. Still nothing run against live GitHub.
