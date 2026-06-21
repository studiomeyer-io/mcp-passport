//! Cross-file consistency tests for `check_consistency`.
//!
//! Each test gets its own scratch directory (unique per process + atomic counter so
//! parallel test threads never collide) that is removed on drop via an RAII guard.
//! The sibling manifest (package.json / Cargo.toml / pyproject.toml) is written into
//! that directory and the consistency checker is pointed at it.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mcp_passport::crossfile::check_consistency;
use mcp_passport::{Level, Report};
use serde_json::{json, Value};

/// A unique temp directory that cleans itself up on drop.
struct Scratch(PathBuf);

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl Scratch {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "mcp-passport-test-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("create scratch dir");
        Scratch(p)
    }

    fn write(&self, name: &str, content: &str) {
        fs::write(self.0.join(name), content).expect("write manifest");
    }

    fn run(&self, server: &Value) -> Report {
        let mut r = Report::default();
        check_consistency(&self.0, server, &mut r);
        r
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn has(r: &Report, rule: &str) -> bool {
    r.findings.iter().any(|f| f.rule == rule)
}

fn level_of(r: &Report, rule: &str) -> Option<Level> {
    r.findings.iter().find(|f| f.rule == rule).map(|f| f.level)
}

/// A server.json declaring a single package of `registry` type.
fn server_with(registry: &str, name: &str, version: &str) -> Value {
    json!({
        "name": name,
        "version": version,
        "packages": [{ "registryType": registry, "identifier": "x" }]
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. npm — mcpName.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_missing_mcp_name_is_error() {
    let d = Scratch::new("npm-missing");
    d.write("package.json", r#"{"name":"x","version":"1.0.0"}"#);
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "npm.mcp_name.missing"));
    assert_eq!(level_of(&r, "npm.mcp_name.missing"), Some(Level::Error));
}

#[test]
fn npm_mcp_name_mismatch_is_error() {
    let d = Scratch::new("npm-mismatch");
    d.write(
        "package.json",
        r#"{"name":"x","version":"1.0.0","mcpName":"io.github.someone-else/srv"}"#,
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "npm.mcp_name.mismatch"));
    assert_eq!(level_of(&r, "npm.mcp_name.mismatch"), Some(Level::Error));
    assert!(!has(&r, "npm.mcp_name.missing"));
}

#[test]
fn npm_mcp_name_match_is_clean() {
    let d = Scratch::new("npm-ok");
    d.write(
        "package.json",
        r#"{"name":"x","version":"1.0.0","mcpName":"io.github.you/srv"}"#,
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. npm — version.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_version_mismatch_is_warning() {
    let d = Scratch::new("npm-ver");
    d.write(
        "package.json",
        r#"{"name":"x","version":"2.0.0","mcpName":"io.github.you/srv"}"#,
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "npm.version.mismatch"));
    assert_eq!(level_of(&r, "npm.version.mismatch"), Some(Level::Warning));
    // mcpName matches, so only the version finding should be present.
    assert!(!has(&r, "npm.mcp_name.mismatch"));
    assert!(!has(&r, "npm.mcp_name.missing"));
}

#[test]
fn npm_version_match_is_clean() {
    let d = Scratch::new("npm-ver-ok");
    d.write(
        "package.json",
        r#"{"name":"x","version":"1.0.0","mcpName":"io.github.you/srv"}"#,
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "npm.version.mismatch"));
}

#[test]
fn npm_version_only_checked_when_both_present() {
    // package.json has no version field → no version comparison, no finding.
    let d = Scratch::new("npm-noversion");
    d.write(
        "package.json",
        r#"{"name":"x","mcpName":"io.github.you/srv"}"#,
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "npm.version.mismatch"));
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. npm — unparsable package.json.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_unparsable_package_json_is_warning() {
    let d = Scratch::new("npm-bad-json");
    d.write("package.json", "{ this is not json ]");
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "npm.package_json.unparsable"));
    assert_eq!(
        level_of(&r, "npm.package_json.unparsable"),
        Some(Level::Warning)
    );
    // No mcpName findings because parsing aborted early.
    assert!(!has(&r, "npm.mcp_name.missing"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. cargo.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn cargo_version_mismatch_is_warning() {
    let d = Scratch::new("cargo-ver");
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"9.9.9\"\n",
    );
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "cargo.version.mismatch"));
    assert_eq!(level_of(&r, "cargo.version.mismatch"), Some(Level::Warning));
}

#[test]
fn cargo_version_match_is_clean() {
    let d = Scratch::new("cargo-ok");
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"1.0.0\"\n",
    );
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "cargo.version.mismatch"));
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn cargo_without_version_key_is_clean() {
    // [package] with no version → path traversal returns early, no finding.
    let d = Scratch::new("cargo-noversion");
    d.write("Cargo.toml", "[package]\nname = \"x\"\n");
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "cargo.version.mismatch"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. pypi.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn pypi_version_mismatch_is_warning() {
    let d = Scratch::new("pypi-ver");
    d.write(
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"3.2.1\"\n",
    );
    let r = d.run(&server_with("pypi", "io.github.you/srv", "1.0.0"));
    assert!(has(&r, "pypi.version.mismatch"));
    assert_eq!(level_of(&r, "pypi.version.mismatch"), Some(Level::Warning));
}

#[test]
fn pypi_version_match_is_clean() {
    let d = Scratch::new("pypi-ok");
    d.write(
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"1.0.0\"\n",
    );
    let r = d.run(&server_with("pypi", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "pypi.version.mismatch"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. no sibling manifest.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_declared_but_no_package_json_is_clean() {
    let d = Scratch::new("npm-no-sibling");
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(r.findings.is_empty());
}

#[test]
fn cargo_declared_but_no_cargo_toml_is_clean() {
    let d = Scratch::new("cargo-no-sibling");
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    assert!(r.findings.is_empty());
}

#[test]
fn pypi_declared_but_no_pyproject_is_clean() {
    let d = Scratch::new("pypi-no-sibling");
    let r = d.run(&server_with("pypi", "io.github.you/srv", "1.0.0"));
    assert!(r.findings.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. registry type not declared → that manifest is never read.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn package_json_ignored_when_no_npm_package_declared() {
    // server declares cargo only — a broken package.json sitting next to it is ignored.
    let d = Scratch::new("npm-not-declared");
    d.write("package.json", "{ broken json");
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"1.0.0\"\n",
    );
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "npm.package_json.unparsable"));
    assert!(!has(&r, "npm.mcp_name.missing"));
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

#[test]
fn cargo_toml_ignored_when_no_cargo_package_declared() {
    let d = Scratch::new("cargo-not-declared");
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"9.9.9\"\n",
    );
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    assert!(!has(&r, "cargo.version.mismatch"));
}

/// A snake_case `registry_type` in server.json means `has_type("npm")` is false (the
/// crossfile checker only looks at camelCase `registryType`), so the npm cross-check
/// does NOT run even though a package.json is present.
#[test]
fn snake_case_npm_registry_type_skips_cross_check() {
    let d = Scratch::new("snake-npm");
    d.write("package.json", r#"{"name":"x","version":"1.0.0"}"#); // no mcpName
    let server = json!({
        "name": "io.github.you/srv",
        "version": "1.0.0",
        "packages": [{ "registry_type": "npm", "identifier": "x" }]
    });
    let r = d.run(&server);
    assert!(
        !has(&r, "npm.mcp_name.missing"),
        "cross-check must not run for snake_case registry_type"
    );
    assert!(r.findings.is_empty(), "{:?}", r.findings);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. finding payload shape for crossfile findings.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_findings_are_attributed_to_package_json_file() {
    let d = Scratch::new("npm-file-attr");
    d.write("package.json", r#"{"name":"x","version":"1.0.0"}"#);
    let r = d.run(&server_with("npm", "io.github.you/srv", "1.0.0"));
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == "npm.mcp_name.missing")
        .unwrap();
    assert_eq!(f.file, "package.json");
    assert_eq!(f.pointer, "/mcpName");
}

#[test]
fn cargo_finding_is_attributed_to_cargo_toml_with_pointer() {
    let d = Scratch::new("cargo-file-attr");
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"9.9.9\"\n",
    );
    let r = d.run(&server_with("cargo", "io.github.you/srv", "1.0.0"));
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == "cargo.version.mismatch")
        .unwrap();
    assert_eq!(f.file, "Cargo.toml");
    assert_eq!(f.pointer, "/package/version");
}

#[test]
fn multiple_registry_types_each_run_their_own_check() {
    // server declares both npm and cargo; both manifests mismatch.
    let d = Scratch::new("multi-registry");
    d.write(
        "package.json",
        r#"{"name":"x","version":"2.0.0","mcpName":"io.github.you/srv"}"#,
    );
    d.write(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"3.0.0\"\n",
    );
    let server = json!({
        "name": "io.github.you/srv",
        "version": "1.0.0",
        "packages": [
            { "registryType": "npm", "identifier": "x" },
            { "registryType": "cargo", "identifier": "x" }
        ]
    });
    let r = d.run(&server);
    assert!(has(&r, "npm.version.mismatch"));
    assert!(has(&r, "cargo.version.mismatch"));
}
