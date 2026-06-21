//! SARIF serialization + Report bookkeeping + human-readable rendering.
//!
//! SARIF: schema version, the error/warning/note level mapping, ruleId echo, the
//! physical (file) + logical (JSON pointer) location split, and the rule-descriptor
//! deduplication (only fired rules appear, once each). Report: count / has_at_least /
//! empty. render: the clean message and severity grouping.

use mcp_passport::rules::{check_server, CURRENT_SCHEMA};
use mcp_passport::sarif::to_sarif;
use mcp_passport::{report::render, Level, Report};
use serde_json::{json, Value};

/// Build a report from raw findings via the public `push` API.
fn report_of(findings: &[(Level, &'static str, &str, &str)]) -> Report {
    let mut r = Report::default();
    for (lvl, rule, file, pointer) in findings {
        r.push(
            *lvl,
            rule,
            *file,
            *pointer,
            "msg",
            "fix",
            "https://example.test/doc",
        );
    }
    r
}

fn invalid_doc() -> Value {
    json!({}) // fires several errors + advisories
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. SARIF top-level shape.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sarif_version_is_2_1_0() {
    let s = to_sarif(&Report::default());
    assert_eq!(s["version"], "2.1.0");
}

#[test]
fn sarif_advertises_the_schema_store_uri() {
    let s = to_sarif(&Report::default());
    assert_eq!(
        s["$schema"],
        "https://json.schemastore.org/sarif-2.1.0.json"
    );
}

#[test]
fn sarif_driver_name_and_information_uri_are_set() {
    let s = to_sarif(&Report::default());
    let driver = &s["runs"][0]["tool"]["driver"];
    assert_eq!(driver["name"], "mcp-passport");
    assert_eq!(
        driver["informationUri"],
        "https://github.com/studiomeyer-io/mcp-passport"
    );
}

#[test]
fn sarif_driver_version_matches_cargo_pkg_version() {
    let s = to_sarif(&Report::default());
    assert_eq!(
        s["runs"][0]["tool"]["driver"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn sarif_empty_report_has_no_results_and_no_rules() {
    let s = to_sarif(&Report::default());
    assert_eq!(s["runs"][0]["results"].as_array().unwrap().len(), 0);
    assert_eq!(
        s["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. SARIF level mapping: error→error, warning→warning, info→note.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sarif_maps_error_level_to_error() {
    let r = report_of(&[(Level::Error, "name.missing", "server.json", "/name")]);
    let s = to_sarif(&r);
    assert_eq!(s["runs"][0]["results"][0]["level"], "error");
}

#[test]
fn sarif_maps_warning_level_to_warning() {
    let r = report_of(&[(Level::Warning, "schema.missing", "server.json", "/$schema")]);
    let s = to_sarif(&r);
    assert_eq!(s["runs"][0]["results"][0]["level"], "warning");
}

#[test]
fn sarif_maps_info_level_to_note() {
    let r = report_of(&[(
        Level::Info,
        "repository.missing",
        "server.json",
        "/repository",
    )]);
    let s = to_sarif(&r);
    assert_eq!(s["runs"][0]["results"][0]["level"], "note");
}

#[test]
fn sarif_maps_all_three_levels_in_one_run() {
    let r = report_of(&[
        (Level::Error, "name.missing", "server.json", "/name"),
        (Level::Warning, "schema.missing", "server.json", "/$schema"),
        (
            Level::Info,
            "repository.missing",
            "server.json",
            "/repository",
        ),
    ]);
    let s = to_sarif(&r);
    let results = s["runs"][0]["results"].as_array().unwrap();
    let levels: Vec<&str> = results
        .iter()
        .map(|x| x["level"].as_str().unwrap())
        .collect();
    assert_eq!(levels, vec!["error", "warning", "note"]);
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. SARIF ruleId + physical/logical location split.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sarif_result_echoes_rule_id() {
    let r = report_of(&[(
        Level::Error,
        "package.transport.missing",
        "server.json",
        "/packages/0/transport",
    )]);
    let s = to_sarif(&r);
    assert_eq!(
        s["runs"][0]["results"][0]["ruleId"],
        "package.transport.missing"
    );
}

#[test]
fn sarif_physical_location_uses_file_logical_uses_pointer() {
    let r = report_of(&[(
        Level::Error,
        "package.registry_type.missing",
        "package.json",
        "/packages/0/registryType",
    )]);
    let s = to_sarif(&r);
    let loc = &s["runs"][0]["results"][0]["locations"][0];
    assert_eq!(
        loc["physicalLocation"]["artifactLocation"]["uri"],
        "package.json"
    );
    assert_eq!(
        loc["logicalLocations"][0]["name"],
        "/packages/0/registryType"
    );
    assert_eq!(loc["logicalLocations"][0]["kind"], "member");
}

#[test]
fn sarif_message_text_includes_message_and_fix() {
    let mut r = Report::default();
    r.push(
        Level::Error,
        "name.missing",
        "server.json",
        "/name",
        "required field `name` is missing",
        "Add a reverse-DNS name.",
        "https://example.test/doc",
    );
    let s = to_sarif(&r);
    let text = s["runs"][0]["results"][0]["message"]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("required field `name` is missing"));
    assert!(text.contains("Fix:"));
    assert!(text.contains("Add a reverse-DNS name."));
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. SARIF rule descriptors: only fired rules, deduplicated, with helpUri.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sarif_descriptors_contain_only_fired_rules() {
    // Build a real report from a bare object and check descriptors == distinct fired rules.
    let mut r = Report::default();
    check_server("server.json", &invalid_doc(), &mut r);
    let s = to_sarif(&r);

    let fired: std::collections::BTreeSet<String> =
        r.findings.iter().map(|f| f.rule.to_string()).collect();
    let descriptors: std::collections::BTreeSet<String> = s["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(descriptors, fired);
    // A rule we know did NOT fire on an empty object must be absent.
    assert!(!descriptors.contains("name.format"));
}

#[test]
fn sarif_descriptors_are_deduplicated_when_a_rule_fires_twice() {
    // Two packages both missing transport → package.transport.missing fires twice,
    // but the descriptor list must contain it exactly once.
    let v = json!({
        "name": "io.github.you/srv",
        "description": "d",
        "version": "1.0.0",
        "$schema": CURRENT_SCHEMA,
        "repository": { "url": "https://github.com/you/srv" },
        "packages": [
            { "registryType": "npm", "identifier": "a", "version": "1.0.0" },
            { "registryType": "npm", "identifier": "b", "version": "1.0.0" }
        ]
    });
    let mut r = Report::default();
    check_server("server.json", &v, &mut r);

    let n_results = r
        .findings
        .iter()
        .filter(|f| f.rule == "package.transport.missing")
        .count();
    assert_eq!(n_results, 2, "expected two transport.missing results");

    let s = to_sarif(&r);
    let n_descriptors = s["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["id"] == "package.transport.missing")
        .count();
    assert_eq!(n_descriptors, 1, "descriptor must be deduplicated");

    // Both results are still present.
    let n_result_entries = s["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["ruleId"] == "package.transport.missing")
        .count();
    assert_eq!(n_result_entries, 2);
}

#[test]
fn sarif_descriptor_carries_help_uri_from_finding_url() {
    let r = report_of(&[(Level::Error, "name.missing", "server.json", "/name")]);
    let s = to_sarif(&r);
    let d = &s["runs"][0]["tool"]["driver"]["rules"][0];
    assert_eq!(d["id"], "name.missing");
    assert_eq!(d["name"], "name.missing");
    assert_eq!(d["helpUri"], "https://example.test/doc");
}

#[test]
fn sarif_is_serializable_to_string() {
    let mut r = Report::default();
    check_server("server.json", &invalid_doc(), &mut r);
    let s = to_sarif(&r);
    let text = serde_json::to_string(&s).expect("serialize SARIF");
    assert!(text.contains("\"version\":\"2.1.0\""));
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Report bookkeeping.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn report_default_is_empty() {
    let r = Report::default();
    assert!(r.findings.is_empty());
    assert_eq!(r.count(Level::Error), 0);
    assert_eq!(r.count(Level::Warning), 0);
    assert_eq!(r.count(Level::Info), 0);
    assert!(!r.has_at_least(Level::Info));
}

#[test]
fn report_count_is_per_level_exact() {
    let r = report_of(&[
        (Level::Error, "e1", "f", "/p"),
        (Level::Error, "e2", "f", "/p"),
        (Level::Warning, "w1", "f", "/p"),
        (Level::Info, "i1", "f", "/p"),
        (Level::Info, "i2", "f", "/p"),
        (Level::Info, "i3", "f", "/p"),
    ]);
    assert_eq!(r.count(Level::Error), 2);
    assert_eq!(r.count(Level::Warning), 1);
    assert_eq!(r.count(Level::Info), 3);
    assert_eq!(r.findings.len(), 6);
}

#[test]
fn has_at_least_respects_severity_ordering() {
    // Only an info finding: at-least-info true, at-least-warning/error false.
    let info = report_of(&[(Level::Info, "i", "f", "/p")]);
    assert!(info.has_at_least(Level::Info));
    assert!(!info.has_at_least(Level::Warning));
    assert!(!info.has_at_least(Level::Error));

    // A warning satisfies at-least-info and at-least-warning, but not error.
    let warn = report_of(&[(Level::Warning, "w", "f", "/p")]);
    assert!(warn.has_at_least(Level::Info));
    assert!(warn.has_at_least(Level::Warning));
    assert!(!warn.has_at_least(Level::Error));

    // An error satisfies all three thresholds.
    let err = report_of(&[(Level::Error, "e", "f", "/p")]);
    assert!(err.has_at_least(Level::Info));
    assert!(err.has_at_least(Level::Warning));
    assert!(err.has_at_least(Level::Error));
}

#[test]
fn level_ordering_is_info_lt_warning_lt_error() {
    assert!(Level::Info < Level::Warning);
    assert!(Level::Warning < Level::Error);
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. render — clean message + grouping.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn render_clean_report_says_publish_ready() {
    let out = render(&Report::default(), "server.json");
    assert!(out.contains("publish-ready"));
    assert!(out.contains("[OK]"));
    assert!(out.contains("server.json"));
}

#[test]
fn render_groups_by_severity_with_counts() {
    let r = report_of(&[
        (Level::Error, "name.missing", "server.json", "/name"),
        (Level::Warning, "schema.missing", "server.json", "/$schema"),
        (
            Level::Info,
            "repository.missing",
            "server.json",
            "/repository",
        ),
    ]);
    let out = render(&r, "server.json");
    assert!(out.contains("3 finding(s)"));
    assert!(out.contains("1 error"));
    assert!(out.contains("1 warning"));
    assert!(out.contains("1 info"));
    assert!(out.contains("ERROR"));
    assert!(out.contains("WARNING"));
    assert!(out.contains("INFO"));
}

#[test]
fn render_orders_error_group_before_warning_before_info() {
    let r = report_of(&[
        (
            Level::Info,
            "repository.missing",
            "server.json",
            "/repository",
        ),
        (Level::Warning, "schema.missing", "server.json", "/$schema"),
        (Level::Error, "name.missing", "server.json", "/name"),
    ]);
    let out = render(&r, "server.json");
    let e = out.find("ERROR").unwrap();
    let w = out.find("WARNING").unwrap();
    let i = out.find("INFO").unwrap();
    assert!(e < w, "ERROR group must precede WARNING");
    assert!(w < i, "WARNING group must precede INFO");
}

#[test]
fn render_includes_pointer_fix_and_see_lines() {
    let mut r = Report::default();
    r.push(
        Level::Error,
        "name.format",
        "server.json",
        "/name",
        "bad name",
        "use reverse dns",
        "https://example.test/doc",
    );
    let out = render(&r, "server.json");
    assert!(out.contains("/name"));
    assert!(out.contains("bad name"));
    assert!(out.contains("fix: use reverse dns"));
    assert!(out.contains("see: https://example.test/doc"));
}

#[test]
fn render_omits_empty_severity_groups() {
    // Only an info finding → no ERROR / WARNING headers.
    let r = report_of(&[(
        Level::Info,
        "repository.missing",
        "server.json",
        "/repository",
    )]);
    let out = render(&r, "server.json");
    assert!(out.contains("INFO"));
    assert!(!out.contains("ERROR"));
    assert!(!out.contains("WARNING"));
}

#[test]
fn render_uses_bare_file_when_pointer_is_empty() {
    // packages_or_remotes.missing has an empty pointer → only the file is shown for it.
    let mut r = Report::default();
    r.push(
        Level::Error,
        "packages_or_remotes.missing",
        "server.json",
        "",
        "neither packages nor remotes",
        "add one",
        "https://example.test/doc",
    );
    let out = render(&r, "server.json");
    assert!(out.contains("neither packages nor remotes"));
    // No stray " /" location separator on the finding line.
    assert!(out.contains("server.json — neither packages nor remotes"));
}
