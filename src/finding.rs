//! A single validation finding.

use serde::Serialize;

/// Severity of a finding, ordered so `--fail-on` thresholds compare naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Advisory; the registry will accept it but it's worth improving.
    Info,
    /// Likely to be rejected or cause a poor listing; should be fixed.
    Warning,
    /// `mcp-publisher` will reject this; publish will fail.
    Error,
}

impl Level {
    /// Short label for human output.
    pub fn label(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }
}

/// One validation finding against a `server.json` (or a cross-checked manifest).
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Severity.
    pub level: Level,
    /// Stable rule class, e.g. `name.format` or `package.registry_type.snake_case`.
    pub rule: &'static str,
    /// File the finding is about (server.json, package.json, …).
    pub file: String,
    /// JSON pointer / location inside the file, e.g. `/packages/0/registryType`.
    pub pointer: String,
    /// What's wrong.
    pub message: String,
    /// What to do about it.
    pub fix: String,
    /// Source to verify against.
    pub url: &'static str,
}

/// Result of a validation run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    /// All findings.
    pub findings: Vec<Finding>,
}

impl Report {
    /// Number of findings at exactly the given level.
    pub fn count(&self, level: Level) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }

    /// Whether any finding is at or above the given level.
    pub fn has_at_least(&self, level: Level) -> bool {
        self.findings.iter().any(|f| f.level >= level)
    }

    /// Push a finding. Internal varargs-style helper — the fields map 1:1 to [`Finding`].
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        level: Level,
        rule: &'static str,
        file: impl Into<String>,
        pointer: impl Into<String>,
        message: impl Into<String>,
        fix: impl Into<String>,
        url: &'static str,
    ) {
        self.findings.push(Finding {
            level,
            rule,
            file: file.into(),
            pointer: pointer.into(),
            message: message.into(),
            fix: fix.into(),
            url,
        });
    }
}
