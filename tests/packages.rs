//! Per-package rule matrix for `check_server` → `check_package`.
//!
//! Each package-level rule is exercised in isolation by mutating only the `packages`
//! array of an otherwise-valid document, so an assertion failure points at exactly one
//! rule. The registry-type-keyed branches (version required for npm but not oci, mcpb
//! requiring fileSha256) get their own accept + reject pairs.

use mcp_passport::rules::{check_server, CURRENT_SCHEMA};
use mcp_passport::{Level, Report};
use serde_json::{json, Value};

fn check(v: &Value) -> Report {
    let mut r = Report::default();
    check_server("server.json", v, &mut r);
    r
}

fn codes(r: &Report) -> Vec<&str> {
    r.findings.iter().map(|f| f.rule).collect()
}

fn has(r: &Report, rule: &str) -> bool {
    r.findings.iter().any(|f| f.rule == rule)
}

fn level_of(r: &Report, rule: &str) -> Option<Level> {
    r.findings.iter().find(|f| f.rule == rule).map(|f| f.level)
}

/// A valid doc whose single package is replaced with `pkg`.
fn with_package(pkg: Value) -> Value {
    json!({
        "$schema": CURRENT_SCHEMA,
        "name": "io.github.you/my-server",
        "description": "Does a thing.",
        "version": "1.0.0",
        "repository": { "url": "https://github.com/you/my-server", "source": "github" },
        "packages": [pkg]
    })
}

/// A complete, valid npm package object.
fn good_npm() -> Value {
    json!({
        "registryType": "npm",
        "identifier": "@you/my-server",
        "version": "1.0.0",
        "transport": { "type": "stdio" }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. registryType.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn package_missing_registry_type_is_error() {
    let r = check(&with_package(json!({
        "identifier": "x", "version": "1.0.0", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.registry_type.missing"));
    assert_eq!(
        level_of(&r, "package.registry_type.missing"),
        Some(Level::Error)
    );
}

#[test]
fn package_unknown_registry_type_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "gem", "identifier": "x", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.registry_type.unknown"));
    assert_eq!(
        level_of(&r, "package.registry_type.unknown"),
        Some(Level::Warning)
    );
}

#[test]
fn package_snake_case_registry_type_is_error() {
    let r = check(&with_package(json!({
        "registry_type": "npm", "identifier": "x", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.registry_type.snake_case"));
    assert_eq!(
        level_of(&r, "package.registry_type.snake_case"),
        Some(Level::Error)
    );
}

#[test]
fn snake_case_registry_type_does_not_also_fire_missing() {
    // When only snake_case is present, the `.missing` branch is suppressed
    // (the code special-cases "has registry_type key").
    let r = check(&with_package(json!({
        "registry_type": "npm", "identifier": "x", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.registry_type.snake_case"));
    assert!(!has(&r, "package.registry_type.missing"));
}

#[test]
fn all_known_registry_types_are_accepted_for_registry_type_rule() {
    for t in ["npm", "pypi", "nuget", "cargo", "oci", "mcpb"] {
        let mut pkg = json!({
            "registryType": t,
            "identifier": "x",
            "version": "1.0.0",
            "transport": { "type": "stdio" }
        });
        if t == "mcpb" {
            pkg["fileSha256"] = json!("a".repeat(64));
        }
        let r = check(&with_package(pkg));
        assert!(
            !has(&r, "package.registry_type.unknown"),
            "type {t} should be known, got {:?}",
            codes(&r)
        );
        assert!(!has(&r, "package.registry_type.missing"), "type {t}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. identifier.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn package_missing_identifier_is_error() {
    let r = check(&with_package(json!({
        "registryType": "npm", "version": "1.0.0", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.identifier.missing"));
    assert_eq!(
        level_of(&r, "package.identifier.missing"),
        Some(Level::Error)
    );
}

#[test]
fn package_empty_identifier_is_error() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "  ", "version": "1.0.0",
        "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.identifier.missing"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. version — required for npm, not required for oci.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn npm_package_missing_version_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "@you/x", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.version.missing"));
    assert_eq!(
        level_of(&r, "package.version.missing"),
        Some(Level::Warning)
    );
}

#[test]
fn pypi_package_missing_version_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "pypi", "identifier": "mypkg", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.version.missing"));
}

#[test]
fn cargo_package_missing_version_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "cargo", "identifier": "mycrate", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.version.missing"));
}

#[test]
fn nuget_package_missing_version_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "nuget", "identifier": "My.Pkg", "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.version.missing"));
}

#[test]
fn oci_package_missing_version_is_not_flagged() {
    // OCI uses the image tag, so version is not required.
    let r = check(&with_package(json!({
        "registryType": "oci", "identifier": "ghcr.io/you/srv:1.0.0",
        "transport": { "type": "stdio" }
    })));
    assert!(!has(&r, "package.version.missing"), "{:?}", codes(&r));
}

#[test]
fn mcpb_package_missing_version_is_not_flagged() {
    // mcpb is not in the VERSIONED set either.
    let r = check(&with_package(json!({
        "registryType": "mcpb", "identifier": "https://x/s.mcpb",
        "fileSha256": "a".repeat(64), "transport": { "type": "stdio" }
    })));
    assert!(!has(&r, "package.version.missing"), "{:?}", codes(&r));
}

#[test]
fn npm_package_with_version_is_clean_of_version_rule() {
    assert!(!has(
        &check(&with_package(good_npm())),
        "package.version.missing"
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. mcpb fileSha256.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn mcpb_without_file_sha256_is_error() {
    let r = check(&with_package(json!({
        "registryType": "mcpb", "identifier": "https://x/s.mcpb",
        "transport": { "type": "stdio" }
    })));
    assert!(has(&r, "package.file_sha256.missing"));
    assert_eq!(
        level_of(&r, "package.file_sha256.missing"),
        Some(Level::Error)
    );
}

#[test]
fn mcpb_with_file_sha256_is_clean() {
    let r = check(&with_package(json!({
        "registryType": "mcpb", "identifier": "https://x/s.mcpb",
        "fileSha256": "a".repeat(64), "transport": { "type": "stdio" }
    })));
    assert!(!has(&r, "package.file_sha256.missing"));
}

#[test]
fn npm_package_is_not_required_to_have_file_sha256() {
    assert!(!has(
        &check(&with_package(good_npm())),
        "package.file_sha256.missing"
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. transport.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn package_missing_transport_is_error() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "@you/x", "version": "1.0.0"
    })));
    assert!(has(&r, "package.transport.missing"));
    assert_eq!(
        level_of(&r, "package.transport.missing"),
        Some(Level::Error)
    );
}

#[test]
fn package_transport_without_type_is_error() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "@you/x", "version": "1.0.0",
        "transport": { "url": "https://x" }
    })));
    assert!(has(&r, "package.transport.type.missing"));
    assert_eq!(
        level_of(&r, "package.transport.type.missing"),
        Some(Level::Error)
    );
}

#[test]
fn package_transport_empty_type_is_error() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "@you/x", "version": "1.0.0",
        "transport": { "type": "  " }
    })));
    assert!(has(&r, "package.transport.type.missing"));
}

#[test]
fn package_transport_unknown_type_is_warning() {
    let r = check(&with_package(json!({
        "registryType": "npm", "identifier": "@you/x", "version": "1.0.0",
        "transport": { "type": "websocket" }
    })));
    assert!(has(&r, "package.transport.type.unknown"));
    assert_eq!(
        level_of(&r, "package.transport.type.unknown"),
        Some(Level::Warning)
    );
}

#[test]
fn all_known_transport_types_are_accepted() {
    for tt in ["stdio", "streamable-http", "sse"] {
        let r = check(&with_package(json!({
            "registryType": "npm", "identifier": "@you/x", "version": "1.0.0",
            "transport": { "type": tt }
        })));
        assert!(!has(&r, "package.transport.type.unknown"), "transport {tt}");
        assert!(!has(&r, "package.transport.type.missing"), "transport {tt}");
        assert!(!has(&r, "package.transport.missing"), "transport {tt}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. package not an object.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn package_that_is_a_string_is_error() {
    let r = check(&with_package(json!("@you/my-server")));
    assert!(has(&r, "package.not_object"));
    assert_eq!(level_of(&r, "package.not_object"), Some(Level::Error));
}

#[test]
fn package_not_object_short_circuits_other_package_rules() {
    // A non-object package emits exactly package.not_object for that entry —
    // no identifier/transport/registryType findings for it.
    let r = check(&with_package(json!(42)));
    let pkg_codes: Vec<&str> = codes(&r)
        .into_iter()
        .filter(|c| c.starts_with("package."))
        .collect();
    assert_eq!(pkg_codes, vec!["package.not_object"]);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. multiple packages — pointers carry the index, each is checked independently.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn multiple_packages_are_each_validated_with_indexed_pointers() {
    let mut v = with_package(good_npm());
    v["packages"] = json!([
        good_npm(),
        json!({ "registryType": "npm", "transport": { "type": "stdio" } }) // missing identifier + version
    ]);
    let r = check(&v);
    // The second package's identifier finding must point at index 1.
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == "package.identifier.missing")
        .expect("identifier.missing should fire for package #1");
    assert_eq!(f.pointer, "/packages/1/identifier");
}

#[test]
fn good_first_package_does_not_mask_bad_second_package() {
    let mut v = with_package(good_npm());
    v["packages"] = json!([good_npm(), json!("not-an-object")]);
    let r = check(&v);
    let f = r
        .findings
        .iter()
        .find(|f| f.rule == "package.not_object")
        .unwrap();
    assert_eq!(f.pointer, "/packages/1");
}
