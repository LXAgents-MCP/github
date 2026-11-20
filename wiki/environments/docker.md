# Docker

The server runs in a container over **stdio** by default, and over HTTP when told to. The image is for a
client that can spawn `docker run -i`, for one that connects to a published port, and for a host that
has no Rust toolchain.

> **This image has not been built.** The environment this was written in had a Docker client and no
> daemon, so neither the `Dockerfile` nor the commands on this page have been run as written. What was
> checked instead: every `COPY` source exists and is not excluded by `.dockerignore`, and the build
> stage's step, `cargo build --release --locked`, is the same build the repository's own checks run; and
> `SIGTERM` is tested. Not checked: the base images, the musl build inside `rust:1-alpine`, the
> `Dockerfile`'s syntax, and `USER nobody` in a real container. Build it once and run the check at the end of this page before relying on it.

## What a container is for here

* **A pinned toolchain.** `rust:1-alpine` instead of whatever is on the host. For exact
  reproducibility, pin the base images by digest.
* **A clean dependency tree.** `cargo build --release --locked` from `Cargo.lock`. `--locked` fails
  rather than resolving something the lockfile does not contain.
* **A build that leaves nothing behind.** A first stage compiles a static musl binary; the image is a
  second stage, bare `alpine:3` (its CA bundle included), that copies only that binary to
  `/usr/local/bin/lxagents-github`, so neither the toolchain nor the sources ship.
* **A non-root process.** The image runs as `USER nobody`.
* **Isolation from the host.** The server reads no host file and runs no command either way; the
  container also keeps it off the host's file system altogether.

What it does not change: the token still reaches GitHub with whatever permissions it was granted, and
it is in the container's environment, where anyone who can `docker inspect` the container sees it.

## Build

```bash
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
docker build -t "lxagents-github:$VERSION" .
```

Tag it with the version in `Cargo.toml` rather than `latest`. The image's job is to be reproducible,
and `latest` is the one tag that cannot be.

## Run

```bash
docker run --rm -i -e GITHUB_API_KEY "lxagents-github:$VERSION"
```

**`-i` is not optional.** Without it Docker attaches no stdin, the server sees it closed and exits
at once, which reads as a broken image rather than a missing flag. **Do not add `-t`:** a terminal
rewrites line endings on the protocol channel.

`-e GITHUB_API_KEY` with no value passes the variable through from the environment of the
`docker` process, so the key is not on the command line, in shell history or in the image. Never put
it in an `ENV` line: a value baked into the image ships to everyone who pulls it.

Over stdio there is nothing to publish. To serve HTTP from the same image, see [Over HTTP](#over-http) below.

## Register it with a client

```json
{
  "mcpServers": {
    "lxagents-github": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-e", "GITHUB_API_KEY", "lxagents-github:<version>"],
      "env": { "GITHUB_API_KEY": "<your GitHub token>" }
    }
  }
}
```

Both are needed: the `env` block gives the variable to the `docker` process, and `-e` passes it on
into the container. Replace `<version>` with the tag you built. The container needs outbound HTTPS to
GitHub, which Docker's default network gives it.

When the client goes away it closes the pipe and the container exits by itself. `docker stop` ends it
at once: the server handles `SIGTERM`, which it has to, because as the container's first process it
would otherwise ignore it and be killed after the grace period.

## Over HTTP

The image's default is stdio, so serving HTTP is a matter of selecting the transport, publishing the port, and
passing both secrets by name, so the values are not on the command line:

```bash
docker run --rm -p 127.0.0.1:3000:3000 -e MCP_TRANSPORT=http \
  -e GITHUB_API_KEY -e MCP_AUTH_TOKEN "lxagents-github:$VERSION"
```

`GITHUB_API_KEY` is the GitHub token the server uses; `MCP_AUTH_TOKEN` is the access token, at least 32
characters. **A container is a server install:** with the access token set, the server inside it listens on every
interface on port 3000 by default, which is what lets a published port reach it, so `HOST` is not set. Without the
token it would listen on the container's own loopback, which `-p` cannot reach, and it says so at start; with
`HOST=0.0.0.0` and no token it refuses to start. A local install with no token is `MCP_TRANSPORT=http target/release/lxagents-github` on the
machine itself, not a container. `127.0.0.1:` in `-p` keeps the port reachable from this host only, and leaving it off publishes the port to the
network. Clients then connect to `http://127.0.0.1:3000/mcp` with `Authorization: Bearer <MCP_AUTH_TOKEN>`; see
[Set up HTTP with two tokens](../guides/setup-http-token.md). Put TLS in front before publishing the port beyond
the host. **This path, like the rest of the image, has not been run in a container.**

## What is in the image

| Path | Why |
|---|---|
| `/usr/local/bin/lxagents-github` | The server, one static binary compiled from `src/` in the build stage, its version fixed in at build time. |
| `/etc/ssl/certs/ca-certificates.crt` | Already in `alpine:3`; the server verifies GitHub's TLS certificate against it. |

`.dockerignore` keeps the rest out of the build context: `test`, a local `target`, `wiki`, `.agents`, `.claude`, `AGENTS.md`, `README.md`,
`.git`, any `.env` file and local noise. **The image therefore cannot run its own test suite** —
`cargo test` needs `test/`, and the suite is a gate on the repository, not on the artifact.

## No `EXPOSE` and no compose file

There is no `EXPOSE`: the default is stdio, which has no port, and the HTTP transport is opt-in with
`MCP_TRANSPORT=http` and published with `-p`; see [Security model](../security/security-model.md). There
is no compose file either: a stdio server is not a service, and `docker compose up` would start it with
no stdin attached, so it would exit at once. A deployment — a registry, an orchestrator, a schedule — is
the owner's decision, not this repository's.

There is also no `ENTRYPOINT`. The whole default is `CMD ["lxagents-github"]`, so a command after the
image name runs as written. An `ENTRYPOINT` of ours would turn `docker run <image> lxagents-github --version`
into `lxagents-github lxagents-github --version`.

## Verifying an image

```bash
docker run --rm "lxagents-github:$VERSION" lxagents-github --version
```

Prints `lxagents-github <version>` on stderr and exits 0. That shows the binary is on the `PATH`, runs on
the runtime image as `nobody`, and carries its version, and it needs no token and no stdin. To check the tools, point
a client at the image as above and list them, or run `cargo test` in a checkout.

## Related pages

* [Local setup](setup.md) — running the server without a container.
* [Environment](env.md) — the variables.
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
* [Connect a client](../guides/connect-a-client.md)
* [Security model](../security/security-model.md) — what a container does and does not change.
