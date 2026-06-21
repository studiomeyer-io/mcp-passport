# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-06-21

Initial release.

### Added

- Validate a `server.json` (file or directory) against the MCP Registry schema: required
  `name` (reverse-DNS `namespace/slug`) + `description` + `version` (semver), `packages`
  or `remotes` present, and per-package `registryType` / `identifier` / `transport` checks
  — including the common snake_case `registry_type` mistake and a missing `fileSha256` on
  `mcpb` packages.
- Cross-file consistency: `mcpName` + `version` agreement between `server.json` and the
  sibling `package.json` (npm), `Cargo.toml` (cargo) or `pyproject.toml` (pypi).
- `$schema` currency check against the 2025-12-11 revision.
- Human / **SARIF 2.1.0** / JSON output, a configurable `--fail-on` severity gate, and a
  reusable composite GitHub Action.

[Unreleased]: https://github.com/studiomeyer-io/mcp-passport/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/studiomeyer-io/mcp-passport/releases/tag/v0.1.0
