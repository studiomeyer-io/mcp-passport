# Security Policy

## Reporting a vulnerability

Please report security issues privately to **security@studiomeyer.io** or via GitHub's
private vulnerability reporting ("Report a vulnerability" in the Security tab). We aim to
acknowledge within 72 hours.

## Scope & intent

`mcp-passport` reads `server.json` and, when present, the sibling `package.json` /
`Cargo.toml` / `pyproject.toml` from a path you give it. It parses them as data, never
executes or imports anything, makes no network calls, and writes nothing back — output goes
to stdout only. Findings are validation hints to check against the linked registry docs.

## Safety properties

- `#![forbid(unsafe_code)]` across the crate.
- Pure local file reads + parsing; no code execution, no network.
- Manifests are parsed leniently — a malformed sibling manifest yields a finding, never a
  panic.
