//! End-to-end tests for `validate` + `resolve_server_json` over the real filesystem.
//!
//! Exercises path resolution (file vs directory), the error paths (missing file, invalid
//! JSON), and full validation runs that combine structural + cross-file checking through
//! the public `validate` entry point.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mcp_passport::{resolve_server_json, validate, Level};

struct Scratch(PathBuf);

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Scratch {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "mcp-passport-e2e-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("create scratch dir");
        Scratch(p)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, content).expect("write file");
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const VALID_JSON: &str = r#"{
  "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
  "name": "io.github.you/my-server",
  "description": "Does a thing.",
  "version": "1.0.0",
  "repository": { "url": "https://github.com/you/my-server", "source": "github" },
  "packages": [{
    "registryType": "npm",
    "identifier": "@you/my-server",
    "version": "1.0.0",
    "transport": { "type": "stdio" }
  }]
}"#;

// ─────────────────────────────────────────────────────────────────────────────
// 8. resolve_server_json.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn resolve_file_path_returns_it_unchanged() {
    let d = Scratch::new("resolve-file");
    let p = d.write("server.json", VALID_JSON);
    let resolved = resolve_server_json(&p).expect("resolve file");
    assert_eq!(resolved, p);
}

#[test]
fn resolve_directory_finds_inner_server_json() {
    let d = Scratch::new("resolve-dir");
    let inner = d.write("server.json", VALID_JSON);
    let resolved = resolve_server_json(&d.0).expect("resolve dir");
    assert_eq!(resolved, inner);
}

#[test]
fn resolve_directory_without_server_json_is_err() {
    let d = Scratch::new("resolve-dir-empty");
    let err = resolve_server_json(&d.0).unwrap_err();
    assert!(err.to_string().contains("no server.json found"), "{err}");
}

#[test]
fn resolve_nonexistent_file_path_is_returned_as_is() {
    // A non-directory path (even if it does not exist) is returned verbatim;
    // the error surfaces later in `validate` when the read fails.
    let p = PathBuf::from("/nonexistent/path/server.json");
    let resolved = resolve_server_json(&p).expect("non-dir path passes through");
    assert_eq!(resolved, p);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. validate — happy path.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_clean_file_has_no_findings() {
    let d = Scratch::new("validate-clean");
    let p = d.write("server.json", VALID_JSON);
    let report = validate(&p).expect("validate ok");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
}

#[test]
fn validate_uses_real_path_in_finding_file_field() {
    let d = Scratch::new("validate-path");
    // Missing description so at least one structural finding fires.
    let p = d.write(
        "server.json",
        r#"{"name":"io.github.you/srv","version":"1.0.0",
            "packages":[{"registryType":"npm","identifier":"x","version":"1.0.0","transport":{"type":"stdio"}}]}"#,
    );
    let report = validate(&p).expect("validate ok");
    let f = report
        .findings
        .iter()
        .find(|f| f.rule == "description.missing")
        .expect("description.missing should fire");
    // The structural findings carry the *real* file path, not the literal "server.json".
    assert_eq!(f.file, p.to_string_lossy());
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. validate — error paths.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_missing_file_is_err() {
    let err = validate(&PathBuf::from("/definitely/not/here/server.json")).unwrap_err();
    assert!(err.to_string().contains("reading"), "{err}");
}

#[test]
fn validate_invalid_json_is_err() {
    let d = Scratch::new("validate-badjson");
    let p = d.write("server.json", "{ not valid json ]");
    let err = validate(&p).unwrap_err();
    assert!(err.to_string().contains("not valid JSON"), "{err}");
}

#[test]
fn validate_empty_file_is_err() {
    let d = Scratch::new("validate-empty");
    let p = d.write("server.json", "");
    assert!(validate(&p).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. validate — full E2E combining structural + cross-file.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_runs_cross_file_check_against_sibling_package_json() {
    let d = Scratch::new("validate-crossfile");
    let p = d.write("server.json", VALID_JSON);
    // Sibling package.json with a *different* mcpName triggers a cross-file error.
    d.write(
        "package.json",
        r#"{"name":"x","version":"1.0.0","mcpName":"io.github.someone-else/srv"}"#,
    );
    let report = validate(&p).expect("validate ok");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == "npm.mcp_name.mismatch"),
        "cross-file mismatch should be reported, got {:?}",
        report.findings.iter().map(|f| f.rule).collect::<Vec<_>>()
    );
}

#[test]
fn validate_clean_with_matching_sibling_manifest_is_clean() {
    let d = Scratch::new("validate-cf-clean");
    let p = d.write("server.json", VALID_JSON);
    d.write(
        "package.json",
        r#"{"name":"@you/my-server","version":"1.0.0","mcpName":"io.github.you/my-server"}"#,
    );
    let report = validate(&p).expect("validate ok");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
}

#[test]
fn validate_combines_structural_and_crossfile_findings() {
    let d = Scratch::new("validate-combined");
    // Bad semver (structural error) + version mismatch via cargo (cross-file warning).
    let p = d.write(
        "server.json",
        r#"{
            "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
            "name": "io.github.you/srv",
            "description": "d",
            "version": "not-semver",
            "repository": { "url": "https://github.com/you/srv" },
            "packages": [{ "registryType": "cargo", "identifier": "srv", "transport": { "type": "stdio" } }]
        }"#,
    );
    d.write(
        "Cargo.toml",
        "[package]\nname = \"srv\"\nversion = \"1.2.3\"\n",
    );
    let report = validate(&p).expect("validate ok");
    let codes: Vec<&str> = report.findings.iter().map(|f| f.rule).collect();
    assert!(codes.contains(&"version.semver"), "{codes:?}");
    // cargo cross-check compares manifest 1.2.3 vs server.json "not-semver" → mismatch.
    assert!(codes.contains(&"cargo.version.mismatch"), "{codes:?}");
}

#[test]
fn validate_report_level_helpers_work_on_real_run() {
    let d = Scratch::new("validate-levels");
    // Only an advisory issue: outdated schema (info), nothing else.
    let p = d.write(
        "server.json",
        r#"{
            "$schema": "https://static.modelcontextprotocol.io/schemas/2025-07-09/server.schema.json",
            "name": "io.github.you/srv",
            "description": "d",
            "version": "1.0.0",
            "repository": { "url": "https://github.com/you/srv" },
            "packages": [{ "registryType": "npm", "identifier": "x", "version": "1.0.0", "transport": { "type": "stdio" } }]
        }"#,
    );
    let report = validate(&p).expect("validate ok");
    assert!(report.has_at_least(Level::Info));
    assert!(!report.has_at_least(Level::Warning));
    assert!(!report.has_at_least(Level::Error));
    assert_eq!(report.count(Level::Info), 1);
    assert_eq!(report.count(Level::Error), 0);
}
