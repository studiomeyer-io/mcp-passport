//! Name-format and semver matrices for `check_server`.
//!
//! These two regex-driven rules carry the most surface area, so each is exercised with
//! a table of accept / reject cases including the leading/trailing punctuation edges.

use mcp_passport::rules::{check_server, CURRENT_SCHEMA};
use mcp_passport::Report;
use serde_json::{json, Value};

fn check(v: &Value) -> Report {
    let mut r = Report::default();
    check_server("server.json", v, &mut r);
    r
}

fn has(r: &Report, rule: &str) -> bool {
    r.findings.iter().any(|f| f.rule == rule)
}

fn valid() -> Value {
    json!({
        "$schema": CURRENT_SCHEMA,
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
    })
}

/// Replace `name`, return whether `name.format` fired.
fn name_format_fires(name: &str) -> bool {
    let mut v = valid();
    v["name"] = json!(name);
    has(&check(&v), "name.format")
}

/// Replace `version`, return whether `version.semver` fired.
fn semver_fires(version: &str) -> bool {
    let mut v = valid();
    v["version"] = json!(version);
    has(&check(&v), "version.semver")
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. name format — accepted shapes.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn name_github_reverse_dns_is_accepted() {
    assert!(!name_format_fires("io.github.you/srv"));
}

#[test]
fn name_simple_com_example_is_accepted() {
    assert!(!name_format_fires("com.example/x"));
}

#[test]
fn name_dotted_namespace_hyphenated_slug_is_accepted() {
    assert!(!name_format_fires("a.b.c/d-e-f"));
}

#[test]
fn name_single_char_namespace_and_slug_is_accepted() {
    assert!(!name_format_fires("a/b"));
}

#[test]
fn name_digits_are_accepted() {
    assert!(!name_format_fires("io.github.user0/server9"));
}

#[test]
fn name_internal_hyphen_in_namespace_is_accepted() {
    assert!(!name_format_fires("io.git-hub.you/srv"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. name format — rejected shapes.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn name_underscore_is_rejected() {
    assert!(name_format_fires("My_Server"));
}

#[test]
fn name_uppercase_namespace_is_rejected() {
    assert!(name_format_fires("UPPER/x"));
}

#[test]
fn name_without_slash_is_rejected() {
    assert!(name_format_fires("noslash"));
}

#[test]
fn name_with_empty_slug_is_rejected() {
    assert!(name_format_fires("ns/"));
}

#[test]
fn name_with_empty_namespace_is_rejected() {
    assert!(name_format_fires("/slug"));
}

#[test]
fn name_with_double_slash_is_rejected() {
    assert!(name_format_fires("ns//slug"));
}

#[test]
fn name_with_space_in_namespace_is_rejected() {
    assert!(name_format_fires("ns name/slug"));
}

#[test]
fn name_with_space_in_slug_is_rejected() {
    assert!(name_format_fires("ns/slug name"));
}

#[test]
fn name_slug_with_dot_is_rejected() {
    // The slug class only allows hyphens, not dots.
    assert!(name_format_fires("ns/slug.x"));
}

#[test]
fn name_slug_with_underscore_is_rejected() {
    assert!(name_format_fires("ns/sl_ug"));
}

// 2. edge — leading / trailing `.` and `-`.

#[test]
fn name_leading_dot_in_namespace_is_rejected() {
    assert!(name_format_fires(".ns/slug"));
}

#[test]
fn name_trailing_dot_in_namespace_is_rejected() {
    assert!(name_format_fires("ns./slug"));
}

#[test]
fn name_leading_hyphen_in_namespace_is_rejected() {
    assert!(name_format_fires("-ns/slug"));
}

#[test]
fn name_trailing_hyphen_in_namespace_is_rejected() {
    assert!(name_format_fires("ns-/slug"));
}

#[test]
fn name_leading_hyphen_in_slug_is_rejected() {
    assert!(name_format_fires("ns/-slug"));
}

#[test]
fn name_trailing_hyphen_in_slug_is_rejected() {
    assert!(name_format_fires("ns/slug-"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. semver — accepted.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn semver_plain_triple_is_accepted() {
    assert!(!semver_fires("1.0.0"));
}

#[test]
fn semver_zero_patch_is_accepted() {
    assert!(!semver_fires("0.0.1"));
}

#[test]
fn semver_prerelease_is_accepted() {
    assert!(!semver_fires("1.2.3-rc.1"));
}

#[test]
fn semver_build_metadata_is_accepted() {
    assert!(!semver_fires("1.0.0+build.5"));
}

#[test]
fn semver_prerelease_and_build_is_accepted() {
    assert!(!semver_fires("1.0.0-alpha+001"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. semver — rejected.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn semver_v_prefix_is_rejected() {
    assert!(semver_fires("v1"));
}

#[test]
fn semver_single_number_is_rejected() {
    assert!(semver_fires("1"));
}

#[test]
fn semver_two_components_is_rejected() {
    assert!(semver_fires("1.0"));
}

#[test]
fn semver_four_components_is_rejected() {
    assert!(semver_fires("1.0.0.0"));
}

#[test]
fn semver_empty_prerelease_is_rejected() {
    assert!(semver_fires("1.0.0-"));
}

#[test]
fn semver_empty_build_is_rejected() {
    assert!(semver_fires("1.0.0+"));
}

#[test]
fn semver_leading_dot_is_rejected() {
    assert!(semver_fires(".1.0"));
}

/// SemVer 2.0.0 forbids leading zeros in the core identifiers — `01.0.0` / `1.02.0`
/// must be rejected (the matcher uses `(0|[1-9]\d*)`).
#[test]
fn semver_leading_zeros_are_rejected() {
    assert!(semver_fires("01.0.0"));
    assert!(semver_fires("1.02.0"));
    assert!(semver_fires("1.0.00"));
    // but a plain zero component is valid
    assert!(!semver_fires("0.0.0"));
    assert!(!semver_fires("1.0.0-rc.1"));
}
