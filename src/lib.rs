//! # mcp-passport
//!
//! Publish-readiness validator for the [MCP Registry](https://registry.modelcontextprotocol.io).
//! Give it a `server.json` (or the directory holding one) and it checks the file against the
//! registry schema — required fields, the reverse-DNS `name`, `packages`/`remotes`, each
//! package's `registryType`/`identifier`/`transport` — and cross-checks `version` + `mcpName`
//! consistency with the sibling `package.json` / `Cargo.toml` / `pyproject.toml`. Run it
//! before `mcp-publisher publish` so the upload doesn't bounce.
//!
//! Companion to the studiomeyer-io MCP stack: [`mcp-armor`](https://github.com/studiomeyer-io/mcp-armor)
//! (runtime), [`mcp-gauntlet`](https://github.com/studiomeyer-io/mcp-gauntlet) (fuzz + load),
//! [`mcp-covenant`](https://github.com/studiomeyer-io/mcp-covenant) (contract) and
//! [`mcp-herald`](https://github.com/studiomeyer-io/mcp-herald) (spec migration). `mcp-passport`
//! is the *publish gate*.
#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod crossfile;
pub mod finding;
pub mod report;
pub mod rules;
pub mod sarif;

pub use finding::{Finding, Level, Report};

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};

/// Resolve a user-supplied path to the `server.json` file. A directory resolves to the
/// `server.json` inside it.
pub fn resolve_server_json(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_dir() {
        let candidate = path.join("server.json");
        if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!("no server.json found in {}", path.display()))
        }
    } else {
        Ok(path.to_path_buf())
    }
}

/// Validate a `server.json` file plus cross-file consistency with manifests in its directory.
pub fn validate(file: &Path) -> anyhow::Result<Report> {
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", file.display()))?;

    let mut report = Report::default();
    let display = file.to_string_lossy().to_string();
    rules::check_server(&display, &value, &mut report);
    if let Some(dir) = file.parent() {
        crossfile::check_consistency(dir, &value, &mut report);
    }
    Ok(report)
}
