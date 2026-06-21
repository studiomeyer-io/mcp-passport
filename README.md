<!-- studiomeyer-mcp-stack-banner:start -->
> **Part of the [StudioMeyer MCP Stack](https://studiomeyer.io)** — Built in Mallorca 🌴 · ⭐ if you use it
<!-- studiomeyer-mcp-stack-banner:end -->

# mcp-passport

[![crates.io](https://img.shields.io/crates/v/mcp-passport.svg)](https://crates.io/crates/mcp-passport)
[![CI](https://github.com/studiomeyer-io/mcp-passport/actions/workflows/ci.yml/badge.svg)](https://github.com/studiomeyer-io/mcp-passport/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/studiomeyer-io/mcp-passport/badge)](https://scorecard.dev/viewer/?uri=github.com/studiomeyer-io/mcp-passport)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Publish-readiness validator for the [MCP Registry](https://registry.modelcontextprotocol.io).**

Publishing a server to the registry means getting a `server.json` exactly right — the reverse-DNS
name, the `packages` block, a `transport`, and an `mcpName` in your `package.json` that matches.
Get one wrong and `mcp-publisher` bounces the upload (or, worse, the listing is subtly broken).

`mcp-passport` is one static binary that checks all of it locally — against the registry schema
**and** across your `server.json` ↔ `package.json` / `Cargo.toml` / `pyproject.toml` — so the
upload goes through the first time.

```text
$ mcp-passport ./my-server
mcp-passport: ./my-server/server.json — 3 finding(s) (2 error, 1 warning, 0 info)

  ERROR (2)
    server.json /name — `name` "My_Weather" is not a valid reverse-DNS namespace/slug
      fix: Use `{namespace}/{slug}`, lowercase, e.g. `io.github.you/my-server`.
      see: https://github.com/modelcontextprotocol/registry/.../generic-server-json.md
    package.json /mcpName — package.json has no `mcpName` — the registry uses it to verify npm ownership
      fix: Add "mcpName": "io.github.you/weather".
      see: https://github.com/modelcontextprotocol/registry/.../quickstart.mdx
  WARNING (1)
    server.json /packages/0/version — package #0 (npm) has no `version` …
# exit code 1 → the job fails
```

When everything checks out:

```text
$ mcp-passport
mcp-passport: ./server.json is publish-ready. [OK]
```

---

## Install

```sh
cargo install mcp-passport
```

Or build from source:

```sh
git clone https://github.com/studiomeyer-io/mcp-passport
cd mcp-passport && cargo build --release   # binary at ./target/release/mcp-passport
```

---

## Use

```sh
mcp-passport                       # validate ./server.json
mcp-passport path/to/server.json   # a specific file
mcp-passport ./my-server           # find + validate server.json in a directory
mcp-passport --fail-on warning     # gate CI on warnings too
mcp-passport --format sarif        # GitHub code scanning
```

Exit code is `1` when a finding at or above `--fail-on` (default `error`) is present, `0` otherwise.

---

## What it checks

**`server.json` structure** (against the [2025-12-11 schema](https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json)):

- required `name` (reverse-DNS `namespace/slug`), `description`, `version` (semver)
- at least one of `packages` / `remotes`
- per package: `registryType` (npm/pypi/nuget/cargo/oci/mcpb), `identifier`, `transport.type`
  (stdio/streamable-http/sse), `version` for versioned registries, `fileSha256` for `mcpb`
- the common **snake_case `registry_type`** mistake (the registry silently ignores it)
- `$schema` present and on the current revision; `repository` present

**Cross-file consistency** (only when the sibling manifest exists):

- npm → `package.json` `mcpName` matches `server.json` `name` (this is how the registry verifies
  ownership), and the versions agree
- cargo → `Cargo.toml` `[package].version` matches
- pypi → `pyproject.toml` `[project].version` matches

Every finding links the registry doc it comes from. `mcp-passport` is a fast local pre-flight —
the authority is `mcp-publisher validate` and the registry itself.

---

## CI

```yaml
# .github/workflows/mcp-registry.yml
name: MCP registry
on: [pull_request]
jobs:
  passport:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: studiomeyer-io/mcp-passport@v0.1.0
        with:
          path: .
          fail-on: error
```

SARIF works too — `mcp-passport --format sarif > passport.sarif` then upload with
`github/codeql-action/upload-sarif` to annotate the PR.

---

## Part of the StudioMeyer MCP toolkit

A small family of focused, production-grade tools for building, shipping and operating MCP servers:

- [mcp-armor](https://github.com/studiomeyer-io/mcp-armor) — runtime defense sidecar
- [mcp-gauntlet](https://github.com/studiomeyer-io/mcp-gauntlet) — pre-deploy fuzz + load testing
- [mcp-covenant](https://github.com/studiomeyer-io/mcp-covenant) — contract & breaking-change detector
- [mcp-herald](https://github.com/studiomeyer-io/mcp-herald) — 2026-07-28 spec migration linter
- **mcp-passport** *(this one)* — the publish gate: is your `server.json` registry-ready?

## License

MIT © [StudioMeyer](https://studiomeyer.io)
