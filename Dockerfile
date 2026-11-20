# syntax=docker/dockerfile:1

# Two stages. The first compiles the server with the Rust toolchain against musl, so the binary is
# static; the second is a bare Alpine image holding that one binary, so neither the toolchain nor
# the sources ship in the image.
FROM rust:1-alpine AS build

WORKDIR /srv

# Dependencies come from the lockfile only: `--locked` fails rather than resolve something the
# lockfile does not contain, which is the point of building an image from a versioned tree.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin lxagents-github

FROM alpine:3

# The base image already carries the CA bundle the server verifies GitHub's certificate against,
# so nothing is installed here.
COPY --from=build /srv/target/release/lxagents-github /usr/local/bin/lxagents-github

# Not root. The process speaks to GitHub and reads nothing else; it needs nothing more.
USER nobody

# No EXPOSE, on purpose: the default is stdio, which has no port, and HTTP is opt-in with
# `-e MCP_TRANSPORT=http`, published by `-p` at run time. A stdio client reaches it through the
# container's stdin and stdout, so it runs with `docker run -i`. Without `-i` Docker attaches no
# stdin, the server sees it closed, and exits at once.
#
# No secret is here either. A value baked into an ENV line ships to everyone who pulls the image;
# both are passed at run time: `-e GITHUB_API_KEY` for the GitHub key, and for HTTP also
# `-e MCP_AUTH_TOKEN`, the access token clients present.
#
# The whole default is CMD and this file sets no ENTRYPOINT, so a command after the image name runs
# as written: `docker run <image> lxagents-github --version`.
CMD ["lxagents-github"]
