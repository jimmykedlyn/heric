//! What a script produced.

use heric_model::Patch;

/// The status of one assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStatus {
    /// The assertion passed.
    Passed,
    /// The assertion failed.
    Failed,
    /// The assertion was skipped with `insomnia.test.skip`.
    Skipped,
}

/// One `insomnia.test` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    /// The test's name, from its first argument.
    pub name: String,
    /// Passed, failed or skipped.
    pub status: TestStatus,
    /// The failure message, when it failed.
    pub error: Option<String>,
}

impl TestResult {
    /// Whether the assertion passed.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.status == TestStatus::Passed
    }
}

/// Everything a run produced.
///
/// Returned instead of mutating the document in place so a script that changes
/// the environment and then throws does not leave a half-applied change: the
/// caller applies [`ScriptOutcome::patches`] only when it wants to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptOutcome {
    /// Field changes the script asked for.
    pub patches: Vec<Patch>,
    /// Assertion results, in the order they ran.
    pub tests: Vec<TestResult>,
    /// Console output, captured for the timeline (`SCR-009`).
    pub console: Vec<crate::ConsoleEntry>,
    /// Cookies the script set.
    pub cookies: Vec<(String, String)>,
    /// Whether the script asked for the request to be skipped
    /// (`insomnia.execution.skipRequest`).
    pub skip_request: bool,
    /// A request the script wants to run next, by name
    /// (`insomnia.execution.setNextRequest`).
    pub next_request: Option<NextRequest>,
}

/// What a script wants to run after this request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextRequest {
    /// By name, resolved by the runner.
    ByName(String),
    /// By id.
    ById(heric_model::Id),
    /// Nothing else.
    Stop,
}

impl ScriptOutcome {
    /// Whether every assertion passed.
    ///
    /// A skipped assertion does not fail the set: `insomnia.test.skip` exists to
    /// mark a test as not applicable, and the runner reports it separately.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.tests
            .iter()
            .all(|test| test.status != TestStatus::Failed)
    }

    /// How many assertions failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.tests
            .iter()
            .filter(|test| test.status == TestStatus::Failed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skipped_test_does_not_fail_the_set() {
        let outcome = ScriptOutcome {
            tests: vec![
                TestResult {
                    name: "a".to_owned(),
                    status: TestStatus::Passed,
                    error: None,
                },
                TestResult {
                    name: "b".to_owned(),
                    status: TestStatus::Skipped,
                    error: None,
                },
            ],
            ..ScriptOutcome::default()
        };
        assert!(outcome.all_passed());
        assert_eq!(outcome.failed(), 0);
    }

    #[test]
    fn a_failed_test_fails_the_set() {
        let outcome = ScriptOutcome {
            tests: vec![TestResult {
                name: "a".to_owned(),
                status: TestStatus::Failed,
                error: Some("expected 200".to_owned()),
            }],
            ..ScriptOutcome::default()
        };
        assert!(!outcome.all_passed());
        assert_eq!(outcome.failed(), 1);
    }
}
