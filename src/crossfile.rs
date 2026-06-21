//! Cross-file consistency: does `server.json` agree with the sibling package manifest?
//!
//! The registry verifies ownership by matching `server.json` `name` against an `mcpName`
//! field the package itself publishes (for npm, in `package.json`). It also expects the
//! `version` to track the published package. These checks run only when both the relevant
//! package type is declared *and* the sibling manifest exists next to `server.json`.

use std::path::Path;

use serde_json::Value;

use crate::finding::{Level, Report};

const DOC_QUICKSTART: &str =
    "https://github.com/modelcontextprotocol/registry/blob/main/docs/modelcontextprotocol-io/quickstart.mdx";

fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// Run all cross-file checks for the server.json located in `dir`.
pub fn check_consistency(dir: &Path, v: &Value, r: &mut Report) {
    let server_name = str_at(v, "name");
    let server_version = str_at(v, "version");
    let packages = v
        .get("packages")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let has_type = |t: &str| {
        packages
            .iter()
            .any(|p| p.get("registryType").and_then(|x| x.as_str()) == Some(t))
    };

    if has_type("npm") {
        check_npm(dir, server_name, server_version, r);
    }
    if has_type("cargo") {
        check_toml_version(
            dir,
            "Cargo.toml",
            &["package", "version"],
            "cargo.version.mismatch",
            "the crate version",
            server_version,
            r,
        );
    }
    if has_type("pypi") {
        check_toml_version(
            dir,
            "pyproject.toml",
            &["project", "version"],
            "pypi.version.mismatch",
            "the published package version",
            server_version,
            r,
        );
    }
}

fn check_npm(dir: &Path, server_name: Option<&str>, server_version: Option<&str>, r: &mut Report) {
    let pj = dir.join("package.json");
    if !pj.exists() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(&pj) else {
        return;
    };
    let Ok(pkg) = serde_json::from_str::<Value>(&text) else {
        r.push(
            Level::Warning,
            "npm.package_json.unparsable",
            "package.json",
            "",
            "found package.json next to server.json but could not parse it",
            "Ensure package.json is valid JSON.",
            DOC_QUICKSTART,
        );
        return;
    };

    match (str_at(&pkg, "mcpName"), server_name) {
        (None, _) => r.push(
            Level::Error,
            "npm.mcp_name.missing",
            "package.json",
            "/mcpName",
            "package.json has no `mcpName` — the registry uses it to verify npm ownership",
            format!(
                "Add \"mcpName\": \"{}\".",
                server_name.unwrap_or("io.github.you/server")
            ),
            DOC_QUICKSTART,
        ),
        (Some(m), Some(n)) if m != n => r.push(
            Level::Error,
            "npm.mcp_name.mismatch",
            "package.json",
            "/mcpName",
            format!("package.json mcpName \"{m}\" does not match server.json name \"{n}\""),
            "Make package.json `mcpName` identical to server.json `name`.",
            DOC_QUICKSTART,
        ),
        _ => {}
    }

    if let (Some(pv), Some(sv)) = (str_at(&pkg, "version"), server_version) {
        if pv != sv {
            r.push(
                Level::Warning,
                "npm.version.mismatch",
                "package.json",
                "/version",
                format!(
                    "package.json version \"{pv}\" does not match server.json version \"{sv}\""
                ),
                "Keep server.json `version` in sync with the published package version.",
                DOC_QUICKSTART,
            );
        }
    }
}

fn check_toml_version(
    dir: &Path,
    filename: &str,
    path: &[&str],
    rule: &'static str,
    what: &str,
    server_version: Option<&str>,
    r: &mut Report,
) {
    let file = dir.join(filename);
    if !file.exists() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(&file) else {
        return;
    };
    let Ok(parsed) = text.parse::<toml::Value>() else {
        return;
    };
    let mut cur = &parsed;
    for key in path {
        match cur.get(key) {
            Some(next) => cur = next,
            None => return,
        }
    }
    if let (Some(mv), Some(sv)) = (cur.as_str(), server_version) {
        if mv != sv {
            r.push(
                Level::Warning,
                rule,
                filename.to_string(),
                format!("/{}", path.join("/")),
                format!("{filename} version \"{mv}\" does not match server.json version \"{sv}\""),
                format!("Keep server.json `version` in sync with {what}."),
                DOC_QUICKSTART,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("passport-{}-{}", tag, std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn write(&self, name: &str, content: &str) {
            fs::write(self.0.join(name), content).unwrap();
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn server(npm: bool) -> Value {
        json!({
            "name": "io.github.you/srv", "version": "1.0.0",
            "packages": [{ "registryType": if npm {"npm"} else {"cargo"}, "identifier": "x" }]
        })
    }

    #[test]
    fn npm_missing_mcp_name_is_error() {
        let d = TempDir::new("npm-missing");
        d.write("package.json", r#"{"name":"x","version":"1.0.0"}"#);
        let mut r = Report::default();
        check_consistency(&d.0, &server(true), &mut r);
        assert!(r.findings.iter().any(|f| f.rule == "npm.mcp_name.missing"));
    }

    #[test]
    fn npm_mcp_name_match_is_clean() {
        let d = TempDir::new("npm-ok");
        d.write(
            "package.json",
            r#"{"name":"x","version":"1.0.0","mcpName":"io.github.you/srv"}"#,
        );
        let mut r = Report::default();
        check_consistency(&d.0, &server(true), &mut r);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }

    #[test]
    fn npm_version_mismatch_is_warning() {
        let d = TempDir::new("npm-ver");
        d.write(
            "package.json",
            r#"{"name":"x","version":"2.0.0","mcpName":"io.github.you/srv"}"#,
        );
        let mut r = Report::default();
        check_consistency(&d.0, &server(true), &mut r);
        assert!(r.findings.iter().any(|f| f.rule == "npm.version.mismatch"));
    }

    #[test]
    fn cargo_version_mismatch_is_warning() {
        let d = TempDir::new("cargo-ver");
        d.write("Cargo.toml", "[package]\nname=\"x\"\nversion=\"9.9.9\"\n");
        let mut r = Report::default();
        check_consistency(&d.0, &server(false), &mut r);
        assert!(r
            .findings
            .iter()
            .any(|f| f.rule == "cargo.version.mismatch"));
    }

    #[test]
    fn no_sibling_manifest_no_findings() {
        let d = TempDir::new("none");
        let mut r = Report::default();
        check_consistency(&d.0, &server(true), &mut r);
        assert!(r.findings.is_empty());
    }
}
