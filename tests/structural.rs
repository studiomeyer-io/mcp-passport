//! Structural validation tests for `check_server` — the pure, file-system-free rule engine.
//!
//! Covers: a fully-valid `server.json` produces zero findings, then each required-field
//! violation in isolation produces exactly the expected rule, plus the packages/remotes
//! matrix, the per-package rule matrix, the `$schema` currency ladder, and the
//! repository recommendation levels.

use mcp_passport::rules::{check_server, CURRENT_SCHEMA};
use mcp_passport::{Level, Report};
use serde_json::{json, Value};

/// Run `check_server` over `v` and return the resulting report.
fn check(v: &Value) -> Report {
    let mut r = Report::default();
    check_server("server.json", v, &mut r);
    r
}

/// All rule ids that fired, in order.
fn codes(r: &Report) -> Vec<&str> {
    r.findings.iter().map(|f| f.rule).collect()
}

/// True iff exactly one finding carries `rule`.
fn fired_once(r: &Report, rule: &str) -> bool {
    r.findings.iter().filter(|f| f.rule == rule).count() == 1
}

/// The level of the (first) finding carrying `rule`, if any.
fn level_of(r: &Report, rule: &str) -> Option<Level> {
    r.findings.iter().find(|f| f.rule == rule).map(|f| f.level)
}

/// A fully-valid server.json that should validate with zero findings.
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

// ─────────────────────────────────────────────────────────────────────────────
// 1. The golden path + each required-field violation in isolation.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fully_valid_server_json_has_zero_findings() {
    let r = check(&valid());
    assert!(r.findings.is_empty(), "expected clean, got {:?}", codes(&r));
}

#[test]
fn fully_valid_server_json_is_not_at_any_level() {
    let r = check(&valid());
    assert!(!r.has_at_least(Level::Info));
    assert!(!r.has_at_least(Level::Warning));
    assert!(!r.has_at_least(Level::Error));
    assert_eq!(r.findings.len(), 0);
}

#[test]
fn top_level_not_object_is_error_and_short_circuits() {
    // A non-object top level produces server.not_object and nothing else.
    for v in [
        json!("a string"),
        json!(42),
        json!(["array"]),
        json!(null),
        json!(true),
    ] {
        let r = check(&v);
        assert_eq!(codes(&r), vec!["server.not_object"], "for input {v}");
    }
}

#[test]
fn missing_name_alone_fires_name_missing() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("name");
    let r = check(&v);
    assert!(fired_once(&r, "name.missing"));
    assert_eq!(level_of(&r, "name.missing"), Some(Level::Error));
    assert!(!codes(&r).contains(&"name.format"));
}

#[test]
fn empty_name_string_fires_name_missing_not_format() {
    let mut v = valid();
    v["name"] = json!("   ");
    let r = check(&v);
    assert!(fired_once(&r, "name.missing"));
    assert!(!codes(&r).contains(&"name.format"));
}

#[test]
fn missing_description_alone_fires_description_missing() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("description");
    let r = check(&v);
    assert_eq!(codes(&r), vec!["description.missing"]);
    assert_eq!(level_of(&r, "description.missing"), Some(Level::Error));
}

#[test]
fn empty_description_fires_description_missing() {
    let mut v = valid();
    v["description"] = json!("");
    assert!(fired_once(&check(&v), "description.missing"));
}

#[test]
fn missing_version_alone_fires_version_missing() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("version");
    let r = check(&v);
    assert_eq!(codes(&r), vec!["version.missing"]);
    assert_eq!(level_of(&r, "version.missing"), Some(Level::Error));
    assert!(!codes(&r).contains(&"version.semver"));
}

#[test]
fn missing_packages_and_remotes_fires_packages_or_remotes_missing() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("packages");
    let r = check(&v);
    assert!(fired_once(&r, "packages_or_remotes.missing"));
    assert_eq!(
        level_of(&r, "packages_or_remotes.missing"),
        Some(Level::Error)
    );
}

#[test]
fn empty_object_reports_all_four_required_errors() {
    let r = check(&json!({}));
    let c = codes(&r);
    assert!(c.contains(&"name.missing"));
    assert!(c.contains(&"description.missing"));
    assert!(c.contains(&"version.missing"));
    assert!(c.contains(&"packages_or_remotes.missing"));
    // $schema + repository are advisory and also fire on a bare object.
    assert!(c.contains(&"schema.missing"));
    assert!(c.contains(&"repository.missing"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. packages / remotes matrix.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn packages_only_does_not_fire_packages_or_remotes_missing() {
    let v = valid(); // has packages, no remotes
    assert!(!codes(&check(&v)).contains(&"packages_or_remotes.missing"));
}

#[test]
fn remotes_only_does_not_fire_packages_or_remotes_missing() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("packages");
    v["remotes"] = json!([{ "type": "streamable-http", "url": "https://x/mcp" }]);
    assert!(!codes(&check(&v)).contains(&"packages_or_remotes.missing"));
}

#[test]
fn both_packages_and_remotes_present_is_accepted() {
    let mut v = valid();
    v["remotes"] = json!([{ "type": "streamable-http", "url": "https://x/mcp" }]);
    assert!(!codes(&check(&v)).contains(&"packages_or_remotes.missing"));
}

#[test]
fn neither_packages_nor_remotes_fires_once() {
    let mut v = valid();
    let obj = v.as_object_mut().unwrap();
    obj.remove("packages");
    obj.remove("remotes");
    assert!(fired_once(&check(&v), "packages_or_remotes.missing"));
}

#[test]
fn empty_arrays_for_both_fire_packages_or_remotes_missing() {
    let mut v = valid();
    v["packages"] = json!([]);
    v["remotes"] = json!([]);
    assert!(fired_once(&check(&v), "packages_or_remotes.missing"));
}

#[test]
fn empty_packages_but_nonempty_remotes_is_accepted() {
    let mut v = valid();
    v["packages"] = json!([]);
    v["remotes"] = json!([{ "type": "sse", "url": "https://x/sse" }]);
    assert!(!codes(&check(&v)).contains(&"packages_or_remotes.missing"));
}

#[test]
fn empty_remotes_but_nonempty_packages_is_accepted() {
    let mut v = valid();
    v["remotes"] = json!([]);
    // packages from valid() is non-empty
    assert!(!codes(&check(&v)).contains(&"packages_or_remotes.missing"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. $schema currency ladder.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn missing_schema_is_warning() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("$schema");
    let r = check(&v);
    assert!(fired_once(&r, "schema.missing"));
    assert_eq!(level_of(&r, "schema.missing"), Some(Level::Warning));
    assert!(!codes(&r).contains(&"schema.outdated"));
}

#[test]
fn outdated_schema_is_info() {
    let mut v = valid();
    v["$schema"] =
        json!("https://static.modelcontextprotocol.io/schemas/2025-07-09/server.schema.json");
    let r = check(&v);
    assert!(fired_once(&r, "schema.outdated"));
    assert_eq!(level_of(&r, "schema.outdated"), Some(Level::Info));
    assert!(!codes(&r).contains(&"schema.missing"));
}

#[test]
fn current_schema_fires_neither_schema_rule() {
    let r = check(&valid());
    assert!(!codes(&r).contains(&"schema.missing"));
    assert!(!codes(&r).contains(&"schema.outdated"));
}

#[test]
fn empty_schema_string_is_treated_as_missing() {
    let mut v = valid();
    v["$schema"] = json!("");
    let r = check(&v);
    assert!(fired_once(&r, "schema.missing"));
    assert!(!codes(&r).contains(&"schema.outdated"));
}

// ─────────────────────────────────────────────────────────────────────────────
// repository recommendation levels.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn missing_repository_is_info() {
    let mut v = valid();
    v.as_object_mut().unwrap().remove("repository");
    let r = check(&v);
    assert!(fired_once(&r, "repository.missing"));
    assert_eq!(level_of(&r, "repository.missing"), Some(Level::Info));
}

#[test]
fn repository_present_without_url_is_warning() {
    let mut v = valid();
    v["repository"] = json!({ "source": "github" });
    let r = check(&v);
    assert!(fired_once(&r, "repository.url.missing"));
    assert_eq!(level_of(&r, "repository.url.missing"), Some(Level::Warning));
    assert!(!codes(&r).contains(&"repository.missing"));
}

#[test]
fn repository_with_empty_url_is_warning() {
    let mut v = valid();
    v["repository"] = json!({ "url": "  ", "source": "github" });
    assert!(fired_once(&check(&v), "repository.url.missing"));
}

#[test]
fn repository_with_url_is_clean() {
    let r = check(&valid());
    assert!(!codes(&r).contains(&"repository.missing"));
    assert!(!codes(&r).contains(&"repository.url.missing"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Finding payload shape — pointers + non-empty messages/fixes.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn name_format_finding_points_at_slash_name() {
    let mut v = valid();
    v["name"] = json!("UPPER/x");
    let r = check(&v);
    let f = r.findings.iter().find(|f| f.rule == "name.format").unwrap();
    assert_eq!(f.pointer, "/name");
    assert_eq!(f.file, "server.json");
    assert!(!f.message.is_empty());
    assert!(!f.fix.is_empty());
    assert!(f.url.starts_with("https://"));
}

#[test]
fn packages_or_remotes_missing_has_empty_pointer() {
    let mut v = valid();
    let obj = v.as_object_mut().unwrap();
    obj.remove("packages");
    obj.remove("remotes");
    let f = check(&v)
        .findings
        .into_iter()
        .find(|f| f.rule == "packages_or_remotes.missing")
        .unwrap();
    assert_eq!(f.pointer, "");
}

#[test]
fn file_label_is_passed_through_to_findings() {
    let mut r = Report::default();
    check_server("custom/path/server.json", &json!({}), &mut r);
    assert!(r
        .findings
        .iter()
        .all(|f| f.file == "custom/path/server.json"));
}
