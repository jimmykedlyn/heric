//! The context a script runs against.

use heric_model::{Doc, Execution, Id};

/// Which script is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPhase {
    /// Before the request is sent.
    PreRequest,
    /// After the response arrived.
    AfterResponse,
}

impl ScriptPhase {
    /// The phase's name, for the timeline.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreRequest => "pre-request",
            Self::AfterResponse => "after-response",
        }
    }
}

/// State a script can read and write.
///
/// Borrowed, not owned, because the script's mutations are collected into a
/// [`crate::ScriptOutcome`] instead of being written straight through: a script
/// that throws halfway must not leave the database in a state the user did not
/// ask for.
pub struct ScriptContext<'a> {
    /// The request being made.
    pub request: &'a Doc,
    /// The response, in an after-response script.
    pub response: Option<&'a Execution>,
    /// The environment in scope for this request, flattened with the seven-scope
    /// precedence already applied (`SCR-003`).
    pub environment: &'a mut std::collections::BTreeMap<String, String>,
    /// The base environment.
    pub base_environment: &'a mut std::collections::BTreeMap<String, String>,
    /// Collection variables.
    pub collection_variables: &'a mut std::collections::BTreeMap<String, String>,
    /// Globals.
    pub globals: &'a mut std::collections::BTreeMap<String, String>,
    /// The base globals.
    pub base_globals: &'a mut std::collections::BTreeMap<String, String>,
    /// Iteration data, when the runner supplied any.
    pub iteration_data: &'a mut std::collections::BTreeMap<String, String>,
    /// Where this request sits in a run, for `insomnia.info`.
    pub info: &'a EventInfo,
    /// Which phase.
    pub phase: ScriptPhase,
}

/// What `insomnia.info` reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInfo {
    /// The event name, `pre-request` or `after-response`.
    pub event_name: String,
    /// The iteration number, starting at 1.
    pub iteration: u32,
    /// How many iterations the run has.
    pub iteration_count: u32,
    /// The request's name.
    pub request_name: String,
    /// The request's id.
    pub request_id: Id,
}

/// Where a script is executing, which changes what is available.
///
/// The original distinguishes the desktop app, the CLI and a plugin sandbox, and
/// the differences are user-visible: vault access and some globals are not
/// available everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionLocation {
    /// The desktop application.
    Application,
    /// The CLI runner.
    Cli,
    /// A test harness, with no network.
    Test,
}

/// The mutable half of a script's state.
///
/// Collected apart from [`ScriptContext`] so the runner can discard them when a
/// script throws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MutableState {
    /// Environment changes, keyed by name.
    pub environment: std::collections::BTreeMap<String, String>,
    /// Base-environment changes.
    pub base_environment: std::collections::BTreeMap<String, String>,
    /// Global changes.
    pub globals: std::collections::BTreeMap<String, String>,
    /// Request changes, keyed by field path.
    pub request: std::collections::BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_keep_their_wire_names() {
        assert_eq!(ScriptPhase::PreRequest.as_str(), "pre-request");
        assert_eq!(ScriptPhase::AfterResponse.as_str(), "after-response");
    }
}
