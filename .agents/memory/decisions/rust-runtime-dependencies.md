---
name: memory-decisions-rust-runtime-dependencies
description: Why the Rust server has more runtime dependencies than the SDK and zod, what each crate is for, and what it replaced in the TypeScript server.
---

# Runtime dependencies of the Rust server

## 2026-11-14

**Decision.** `.agents/rules/repository.md` allows two runtime dependencies, the MCP SDK and zod, and says a
third needs a record before it lands. Node gave the TypeScript server an HTTP client, TLS, an event loop, JSON,
hashing and base64 for free; Rust's standard library has none of them, so the set grows. Each crate below
replaces something Node provided, and none is there to serve one call site.

| Crate | Why | Replaces |
|---|---|---|
| `rmcp` (`server`, `transport-io`, `transport-streamable-http-server`) | The MCP SDK the owner asked for | `@modelcontextprotocol/sdk` |
| `axum` | The web framework the owner asked for: the two routes and the guard in front of them | `node:http` |
| `tokio` | Async runtime: stdio, TCP, signals, timers | Node's event loop |
| `reqwest` (rustls, no default features) | HTTPS to `api.github.com`, a timeout, a streamed size cap, redirects that drop `Authorization` on a change of host | `fetch` |
| `serde`, `serde_json` (`preserve_order`) | JSON; `preserve_order` keeps a payload's field order, which the rendered text follows | `JSON` |
| `sha2`, `subtle` | SHA-256 digests of the access token, compared in constant time | `createHash`, `timingSafeEqual` |
| `base64` | File content in and out | `Buffer` |
| `percent-encoding` | `encodeURIComponent`, with exactly its set | `encodeURIComponent` |
| `regex` | The few patterns the server matches; linear time | `RegExp` |
| `chrono` | Date checks and the rate-limit reset time | `Date` |
| `encoding_rs` | The Windows-1252 fallback when a file is not UTF-8 | `TextDecoder` |
| `futures` | Catching a panic in a handler future | — |
| `tower`, `http-body-util`, `bytes` | Handing a checked request on to the `rmcp` service | — |

zod has no counterpart: the schemas are JSON in each tool's module, pinned by the golden `tools/list`.

**What it costs.** A larger `Cargo.lock` to keep current, and a supply chain to watch with `cargo audit`. A
crate added beyond these still needs its own record first.
