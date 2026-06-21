//! Human-readable terminal rendering.

use crate::finding::{Level, Report};

/// Render a validation report grouped by severity, most severe first.
pub fn render(report: &Report, target: &str) -> String {
    if report.findings.is_empty() {
        return format!("mcp-passport: {target} is publish-ready. [OK]\n");
    }
    let mut out = String::new();
    out.push_str(&format!(
        "mcp-passport: {} — {} finding(s) ({} error, {} warning, {} info)\n\n",
        target,
        report.findings.len(),
        report.count(Level::Error),
        report.count(Level::Warning),
        report.count(Level::Info),
    ));
    for level in [Level::Error, Level::Warning, Level::Info] {
        let group: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.level == level)
            .collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "  {} ({})\n",
            level.label().to_uppercase(),
            group.len()
        ));
        for f in group {
            let loc = if f.pointer.is_empty() {
                f.file.clone()
            } else {
                format!("{} {}", f.file, f.pointer)
            };
            out.push_str(&format!("    {loc} — {}\n", f.message));
            out.push_str(&format!("      fix: {}\n", f.fix));
            out.push_str(&format!("      see: {}\n", f.url));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_report_message() {
        let out = render(&Report::default(), "server.json");
        assert!(out.contains("publish-ready"));
    }

    #[test]
    fn groups_by_severity() {
        let mut r = Report::default();
        r.push(
            Level::Error,
            "name.missing",
            "server.json",
            "/name",
            "missing name",
            "add it",
            "https://x",
        );
        let out = render(&r, "server.json");
        assert!(out.contains("ERROR"));
        assert!(out.contains("/name"));
        assert!(out.contains("fix:"));
    }
}
