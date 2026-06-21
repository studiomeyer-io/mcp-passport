# Contributing to mcp-passport

Thanks for considering a contribution. `mcp-passport` checks whether an MCP server is ready
to publish to the [registry](https://registry.modelcontextprotocol.io), so a rule earns its
place by mapping to a real registry requirement and shipping with a passing and a clean test.

## Quick Start

```sh
git clone https://github.com/studiomeyer-io/mcp-passport
cd mcp-passport
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
# try it:
cargo run -- path/to/server.json
```

MSRV is **Rust 1.86** — CI checks it on a pinned 1.86 toolchain plus stable.

## Adding a rule

- **Structural rules** (one `server.json`) live in [`src/rules.rs`](src/rules.rs): push a
  `Finding` with a stable rule id, a JSON pointer, a message, a fix, and the registry doc
  URL it derives from.
- **Cross-file rules** (consistency with a sibling manifest) live in
  [`src/crossfile.rs`](src/crossfile.rs).

Add a unit test in the same file: an input that **must** fire and a realistic compliant
input that **must not**. Cross-file tests use a temp dir (see the existing `TempDir` helper).

## Principles

- **Match the registry, don't invent.** Every rule cites the official server.json docs.
  `mcp-passport` is a fast local pre-flight, not a second source of truth — the authority is
  `mcp-publisher validate` and the registry itself.
- **Severity = consequence.** `error` = the publish will be rejected; `warning` = likely to
  cause problems or a poor listing; `info` = advisory.
- **Low noise.** A compliant `server.json` must produce zero findings.
