//! Structural validation of a `server.json` against the MCP Registry schema.
//!
//! Operates on raw [`serde_json::Value`] (not a typed struct) so it can catch malformed
//! input the registry rejects — wrong types, a snake_case `registry_type`, a missing
//! required field — rather than failing to deserialize. Every rule links the registry docs.

use regex::Regex;
use serde_json::Value;

use crate::finding::{Level, Report};

/// The current published schema URL (revision 2025-12-11).
pub const CURRENT_SCHEMA: &str =
    "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json";

const DOC_GENERIC: &str =
    "https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/server-json/generic-server-json.md";
const DOC_REQ: &str =
    "https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/server-json/official-registry-requirements.md";

const ALLOWED_REGISTRY: &[&str] = &["npm", "pypi", "nuget", "cargo", "oci", "mcpb"];
const ALLOWED_TRANSPORT: &[&str] = &["stdio", "streamable-http", "sse"];
/// Registry types whose packages carry an explicit `version` (OCI uses the image tag).
const VERSIONED: &[&str] = &["npm", "pypi", "nuget", "cargo"];

/// A non-empty trimmed string at `key`, if present.
fn nonempty_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// The reverse-DNS `namespace/slug` name shape (lowercase).
fn name_re() -> Regex {
    Regex::new(r"^[a-z0-9]([a-z0-9.-]*[a-z0-9])?/[a-z0-9]([a-z0-9-]*[a-z0-9])?$").unwrap()
}

/// A SemVer 2.0.0 core matcher: MAJOR.MINOR.PATCH (no leading zeros) with optional
/// pre-release / build metadata.
fn semver_re() -> Regex {
    Regex::new(
        r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$",
    )
    .unwrap()
}

/// Validate a parsed `server.json`.
pub fn check_server(file: &str, v: &Value, r: &mut Report) {
    if v.as_object().is_none() {
        r.push(
            Level::Error,
            "server.not_object",
            file,
            "",
            "server.json is not a JSON object",
            "The top level must be a JSON object.",
            DOC_GENERIC,
        );
        return;
    }

    // name (required) + format
    match nonempty_str(v, "name") {
        None => r.push(
            Level::Error,
            "name.missing",
            file,
            "/name",
            "required field `name` is missing or empty",
            "Add a reverse-DNS name like `io.github.you/my-server`.",
            DOC_GENERIC,
        ),
        Some(name) => {
            if !name_re().is_match(name) {
                r.push(
                    Level::Error,
                    "name.format",
                    file,
                    "/name",
                    format!("`name` \"{name}\" is not a valid reverse-DNS namespace/slug"),
                    "Use `{namespace}/{slug}`, lowercase, e.g. `io.github.you/my-server`.",
                    DOC_GENERIC,
                );
            }
        }
    }

    // description (required)
    if nonempty_str(v, "description").is_none() {
        r.push(
            Level::Error,
            "description.missing",
            file,
            "/description",
            "required field `description` is missing or empty",
            "Add a one-line description of what the server does.",
            DOC_GENERIC,
        );
    }

    // version (required) + semver
    match nonempty_str(v, "version") {
        None => r.push(
            Level::Error,
            "version.missing",
            file,
            "/version",
            "required field `version` is missing or empty",
            "Add a semver version, e.g. `1.0.0`.",
            DOC_GENERIC,
        ),
        Some(ver) => {
            if !semver_re().is_match(ver) {
                r.push(
                    Level::Error,
                    "version.semver",
                    file,
                    "/version",
                    format!("`version` \"{ver}\" is not valid semver"),
                    "Use MAJOR.MINOR.PATCH, e.g. `1.0.0`.",
                    DOC_GENERIC,
                );
            }
        }
    }

    // $schema (recommended) + currency
    match nonempty_str(v, "$schema") {
        None => r.push(
            Level::Warning,
            "schema.missing",
            file,
            "/$schema",
            "no `$schema` set",
            format!("Add \"$schema\": \"{CURRENT_SCHEMA}\"."),
            DOC_GENERIC,
        ),
        Some(s) if s != CURRENT_SCHEMA => r.push(
            Level::Info,
            "schema.outdated",
            file,
            "/$schema",
            format!("`$schema` is not the current schema revision ({s})"),
            format!("Point it at {CURRENT_SCHEMA}."),
            DOC_GENERIC,
        ),
        _ => {}
    }

    // packages XOR/AND remotes — at least one non-empty
    let packages = v.get("packages").and_then(|x| x.as_array());
    let has_remotes = v
        .get("remotes")
        .and_then(|x| x.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_packages = packages.map(|a| !a.is_empty()).unwrap_or(false);
    if !has_packages && !has_remotes {
        r.push(
            Level::Error,
            "packages_or_remotes.missing",
            file,
            "",
            "server declares neither `packages` nor `remotes`",
            "Add at least one package (how to install it) or one remote (a hosted endpoint).",
            DOC_REQ,
        );
    }
    if let Some(pkgs) = packages {
        for (i, p) in pkgs.iter().enumerate() {
            check_package(file, i, p, r);
        }
    }

    // repository (recommended)
    match v.get("repository") {
        None => r.push(
            Level::Info,
            "repository.missing",
            file,
            "/repository",
            "no `repository` set",
            "Add { \"url\": \"https://github.com/...\", \"source\": \"github\" } so users can find the source.",
            DOC_GENERIC,
        ),
        Some(repo) => {
            if nonempty_str(repo, "url").is_none() {
                r.push(
                    Level::Warning,
                    "repository.url.missing",
                    file,
                    "/repository/url",
                    "`repository` is present but has no `url`",
                    "Add the repository URL.",
                    DOC_GENERIC,
                );
            }
        }
    }
}

fn check_package(file: &str, i: usize, p: &Value, r: &mut Report) {
    let base = format!("/packages/{i}");
    let po = match p.as_object() {
        Some(o) => o,
        None => {
            r.push(
                Level::Error,
                "package.not_object",
                file,
                base,
                format!("package #{i} is not an object"),
                "Each entry in `packages` must be an object.",
                DOC_GENERIC,
            );
            return;
        }
    };

    // common snake_case mistake
    if po.contains_key("registry_type") && !po.contains_key("registryType") {
        r.push(
            Level::Error,
            "package.registry_type.snake_case",
            file,
            format!("{base}/registry_type"),
            format!("package #{i} uses snake_case `registry_type`"),
            "Rename to camelCase `registryType` — the registry ignores the snake_case form.",
            DOC_GENERIC,
        );
    }

    // registryType
    let rt = nonempty_str(p, "registryType");
    match rt {
        None if !po.contains_key("registry_type") => r.push(
            Level::Error,
            "package.registry_type.missing",
            file,
            format!("{base}/registryType"),
            format!("package #{i} has no `registryType`"),
            format!("Set one of: {}.", ALLOWED_REGISTRY.join(", ")),
            DOC_GENERIC,
        ),
        Some(t) if !ALLOWED_REGISTRY.contains(&t) => r.push(
            Level::Warning,
            "package.registry_type.unknown",
            file,
            format!("{base}/registryType"),
            format!("package #{i} registryType \"{t}\" is not a known type"),
            format!("Use one of: {}.", ALLOWED_REGISTRY.join(", ")),
            DOC_GENERIC,
        ),
        _ => {}
    }

    // identifier
    if nonempty_str(p, "identifier").is_none() {
        r.push(
            Level::Error,
            "package.identifier.missing",
            file,
            format!("{base}/identifier"),
            format!("package #{i} has no `identifier`"),
            "Set the package id (npm `@scope/name`, cargo crate, pypi name, oci `repo:tag`).",
            DOC_GENERIC,
        );
    }

    // version + mcpb file hash, keyed on registry type
    if let Some(t) = rt {
        if VERSIONED.contains(&t) && nonempty_str(p, "version").is_none() {
            r.push(
                Level::Warning,
                "package.version.missing",
                file,
                format!("{base}/version"),
                format!("package #{i} ({t}) has no `version`"),
                "Pin the published package version.",
                DOC_GENERIC,
            );
        }
        if t == "mcpb" && nonempty_str(p, "fileSha256").is_none() {
            r.push(
                Level::Error,
                "package.file_sha256.missing",
                file,
                format!("{base}/fileSha256"),
                format!("package #{i} is `mcpb` but has no `fileSha256`"),
                "mcpb packages must include the file's SHA-256.",
                DOC_REQ,
            );
        }
    }

    // transport
    match p.get("transport") {
        None => r.push(
            Level::Error,
            "package.transport.missing",
            file,
            format!("{base}/transport"),
            format!("package #{i} has no `transport`"),
            "Add { \"type\": \"stdio\" } or { \"type\": \"streamable-http\" }.",
            DOC_GENERIC,
        ),
        Some(t) => match nonempty_str(t, "type") {
            None => r.push(
                Level::Error,
                "package.transport.type.missing",
                file,
                format!("{base}/transport/type"),
                format!("package #{i} transport has no `type`"),
                format!("Set type to one of: {}.", ALLOWED_TRANSPORT.join(", ")),
                DOC_GENERIC,
            ),
            Some(tt) if !ALLOWED_TRANSPORT.contains(&tt) => r.push(
                Level::Warning,
                "package.transport.type.unknown",
                file,
                format!("{base}/transport/type"),
                format!("package #{i} transport type \"{tt}\" is not recognized"),
                format!("Use one of: {}.", ALLOWED_TRANSPORT.join(", ")),
                DOC_GENERIC,
            ),
            _ => {}
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn check(v: &Value) -> Report {
        let mut r = Report::default();
        check_server("server.json", v, &mut r);
        r
    }
    fn codes(r: &Report) -> Vec<&str> {
        r.findings.iter().map(|f| f.rule).collect()
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

    #[test]
    fn a_valid_server_json_has_no_findings() {
        let r = check(&valid());
        assert!(r.findings.is_empty(), "{:?}", codes(&r));
    }

    #[test]
    fn missing_required_fields_are_errors() {
        let r = check(&json!({}));
        let c = codes(&r);
        assert!(c.contains(&"name.missing"));
        assert!(c.contains(&"description.missing"));
        assert!(c.contains(&"version.missing"));
        assert!(c.contains(&"packages_or_remotes.missing"));
    }

    #[test]
    fn bad_name_format_is_error() {
        let mut v = valid();
        v["name"] = json!("My_Server");
        assert!(codes(&check(&v)).contains(&"name.format"));
    }

    #[test]
    fn bad_semver_is_error() {
        let mut v = valid();
        v["version"] = json!("v1");
        assert!(codes(&check(&v)).contains(&"version.semver"));
    }

    #[test]
    fn snake_case_registry_type_is_flagged() {
        let mut v = valid();
        v["packages"] =
            json!([{ "registry_type": "npm", "identifier": "x", "transport": {"type":"stdio"} }]);
        assert!(codes(&check(&v)).contains(&"package.registry_type.snake_case"));
    }

    #[test]
    fn remotes_only_is_valid() {
        let v = json!({
            "name": "io.github.you/srv", "description": "d", "version": "1.0.0",
            "remotes": [{ "type": "streamable-http", "url": "https://x/mcp" }]
        });
        let r = check(&v);
        assert!(!codes(&r).contains(&"packages_or_remotes.missing"));
    }

    #[test]
    fn mcpb_without_sha_is_error() {
        let mut v = valid();
        v["packages"] = json!([{ "registryType": "mcpb", "identifier": "https://x/s.mcpb", "transport": {"type":"stdio"} }]);
        assert!(codes(&check(&v)).contains(&"package.file_sha256.missing"));
    }

    #[test]
    fn missing_transport_is_error() {
        let mut v = valid();
        v["packages"] = json!([{ "registryType": "npm", "identifier": "x", "version": "1.0.0" }]);
        assert!(codes(&check(&v)).contains(&"package.transport.missing"));
    }

    #[test]
    fn outdated_schema_is_info() {
        let mut v = valid();
        v["$schema"] =
            json!("https://static.modelcontextprotocol.io/schemas/2025-07-09/server.schema.json");
        assert!(codes(&check(&v)).contains(&"schema.outdated"));
    }
}
