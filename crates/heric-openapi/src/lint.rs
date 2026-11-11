//! Linting: the ruleset model and the report.
//!
//! Spectral is a JavaScript linter. Heric does not run it: the rules that matter
//! to an API client are the ones that decide whether the document is usable, and
//! a rule engine written in Rust covers them without a Node runtime. What Heric
//! does keep is the shape of the output, because a user who has learned to read
//! Spectral's report should not have to learn a second one.

/// How serious a violation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational.
    Hint,
    /// A warning.
    Warning,
    /// An error, which makes the CLI exit non-zero.
    Error,
}

impl Severity {
    /// The severity as written.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hint => "hint",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// One violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintViolation {
    /// The rule's name, `operation-description` and the like.
    pub rule: String,
    /// How serious.
    pub severity: Severity,
    /// The message.
    pub message: String,
    /// A JSON pointer to what it concerns, so an editor can jump there.
    pub path: String,
    /// The line, when the linter could compute one.
    pub line: Option<u32>,
}

/// A lint run's result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintReport {
    /// The violations, most serious first.
    pub violations: Vec<LintViolation>,
}

impl LintReport {
    /// Whether any violation is an error.
    ///
    /// The CLI exits non-zero only on an error, not on a warning (`IO-036`).
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.violations
            .iter()
            .any(|violation| violation.severity == Severity::Error)
    }

    /// How many violations of a severity were found.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.violations
            .iter()
            .filter(|violation| violation.severity == severity)
            .count()
    }

    /// Whether the run was clean.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Sorts violations by severity, most serious first, keeping the document
    /// order within a severity so two runs of the same rule report identically.
    pub fn sort(&mut self) {
        self.violations.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.path.cmp(&b.path))
        });
    }
}

/// A ruleset to lint against.
#[derive(Debug, Clone, Default)]
pub struct Ruleset {
    /// The rules that are on, by name.
    pub enabled: Vec<String>,
    /// The rules that are off, by name.
    pub disabled: Vec<String>,
    /// Severity overrides, keyed by rule name.
    pub overrides: Vec<(String, Severity)>,
}

impl Ruleset {
    /// The default ruleset.
    #[must_use]
    pub fn recommended() -> Self {
        Self {
            enabled: [
                "operation-operationId",
                "operation-description",
                "oas3-valid-schema",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            disabled: Vec::new(),
            overrides: Vec::new(),
        }
    }

    /// The effective severity of a rule, or `None` when it is off.
    #[must_use]
    pub fn severity_of(&self, rule: &str, default: Severity) -> Option<Severity> {
        if self.disabled.iter().any(|name| name == rule) {
            return None;
        }
        if let Some((_, severity)) = self.overrides.iter().find(|(name, _)| name == rule) {
            return Some(*severity);
        }
        if self.enabled.iter().any(|name| name == rule) {
            return Some(default);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violation(severity: Severity, path: &str) -> LintViolation {
        LintViolation {
            rule: "r".to_owned(),
            severity,
            message: "m".to_owned(),
            path: path.to_owned(),
            line: None,
        }
    }

    #[test]
    fn only_an_error_fails_the_run() {
        let report = LintReport {
            violations: vec![violation(Severity::Warning, "/a")],
        };
        assert!(!report.has_errors());
        assert!(!report.is_clean());
        assert_eq!(report.count(Severity::Warning), 1);
    }

    #[test]
    fn sorting_puts_errors_first_and_is_stable_within_a_severity() {
        let mut report = LintReport {
            violations: vec![
                violation(Severity::Hint, "/z"),
                violation(Severity::Error, "/b"),
                violation(Severity::Error, "/a"),
            ],
        };
        report.sort();
        assert_eq!(report.violations[0].path, "/a");
        assert_eq!(report.violations[1].path, "/b");
        assert_eq!(report.violations[2].severity, Severity::Hint);
    }

    #[test]
    fn a_disabled_rule_has_no_severity() {
        let ruleset = Ruleset::recommended();
        assert_eq!(
            ruleset.severity_of("operation-description", Severity::Warning),
            Some(Severity::Warning)
        );
        assert_eq!(ruleset.severity_of("unknown-rule", Severity::Warning), None);

        let mut ruleset = Ruleset::recommended();
        ruleset.disabled.push("operation-description".to_owned());
        assert_eq!(
            ruleset.severity_of("operation-description", Severity::Warning),
            None
        );
    }

    #[test]
    fn an_override_wins_over_the_default() {
        let mut ruleset = Ruleset::recommended();
        ruleset
            .overrides
            .push(("operation-description".to_owned(), Severity::Error));
        assert_eq!(
            ruleset.severity_of("operation-description", Severity::Warning),
            Some(Severity::Error)
        );
    }
}
