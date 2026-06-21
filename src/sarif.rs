//! SARIF 2.1.0 output for GitHub code scanning.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::finding::{Level, Report};

const INFO_URI: &str = "https://github.com/studiomeyer-io/mcp-passport";

fn level_str(l: Level) -> &'static str {
    match l {
        Level::Error => "error",
        Level::Warning => "warning",
        Level::Info => "note",
    }
}

/// Render a validation report as SARIF. The file is the physical location; the JSON pointer
/// is attached as a logical location.
pub fn to_sarif(report: &Report) -> Value {
    let mut fired: BTreeMap<&str, &'static str> = BTreeMap::new();
    for f in &report.findings {
        fired.insert(f.rule, f.url);
    }
    let rules: Vec<Value> = fired
        .iter()
        .map(|(id, url)| json!({ "id": id, "name": id, "helpUri": url }))
        .collect();

    let results: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.rule,
                "level": level_str(f.level),
                "message": { "text": format!("{} Fix: {}", f.message, f.fix) },
                "locations": [{
                    "physicalLocation": { "artifactLocation": { "uri": f.file } },
                    "logicalLocations": [{ "name": f.pointer, "kind": "member" }]
                }]
            })
        })
        .collect();

    json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "mcp-passport",
                    "informationUri": INFO_URI,
                    "version": env!("CARGO_PKG_VERSION"),
                    "rules": rules
                }
            },
            "results": results
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sarif_shape_and_level_mapping() {
        let mut r = Report::default();
        r.push(
            Level::Error,
            "name.missing",
            "server.json",
            "/name",
            "missing",
            "fix it",
            "https://x",
        );
        let s = to_sarif(&r);
        assert_eq!(s["version"], "2.1.0");
        assert_eq!(s["runs"][0]["tool"]["driver"]["name"], "mcp-passport");
        assert_eq!(s["runs"][0]["results"][0]["ruleId"], "name.missing");
        assert_eq!(s["runs"][0]["results"][0]["level"], "error");
        assert_eq!(
            s["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            "server.json"
        );
    }
}
