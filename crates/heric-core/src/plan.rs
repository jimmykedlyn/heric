//! The send pipeline, as an ordered plan.
//!
//! The original's send pipeline is a long linear function that does eleven
//! things: collect inheritance, render, run the pre-request script, build the
//! body, resolve auth, apply plugin hooks, execute, store the response, run the
//! after-response script, patch the response, record history (`HTTP-001`). It is
//! modelled here as an explicit list of stages so each one can be tested, timed
//! and reported on its own, and so the GUI can show which stage a slow request is
//! in.

use heric_model::{Execution, Id, TimelineEntry};

/// One stage of a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStage {
    /// Merge folder and collection settings into the request.
    Inherit,
    /// Render every templated field.
    Render,
    /// Run the pre-request script.
    PreRequestScript,
    /// Build the wire body.
    BuildBody,
    /// Resolve credentials and apply authentication.
    Authenticate,
    /// Apply plugin hooks, which Heric only supports through the script surface.
    Plugins,
    /// Execute the request.
    Execute,
    /// Store the response and its timeline.
    StoreResponse,
    /// Run the after-response script.
    AfterResponseScript,
    /// Let the script's response patch take effect.
    PatchResponse,
    /// Append to history.
    RecordHistory,
}

impl PlanStage {
    /// The stage's name, for the UI's progress display and the timeline.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::Render => "render",
            Self::PreRequestScript => "pre-request script",
            Self::BuildBody => "build body",
            Self::Authenticate => "authenticate",
            Self::Plugins => "plugins",
            Self::Execute => "execute",
            Self::StoreResponse => "store response",
            Self::AfterResponseScript => "after-response script",
            Self::PatchResponse => "patch response",
            Self::RecordHistory => "record history",
        }
    }

    /// The stages in the order they run.
    pub const ORDER: [Self; 11] = [
        Self::Inherit,
        Self::Render,
        Self::PreRequestScript,
        Self::BuildBody,
        Self::Authenticate,
        Self::Plugins,
        Self::Execute,
        Self::StoreResponse,
        Self::AfterResponseScript,
        Self::PatchResponse,
        Self::RecordHistory,
    ];
}

/// What a send produced.
#[derive(Debug, Clone, Default)]
pub struct SendOutcome {
    /// The response, when the request reached the server.
    ///
    /// `None` means the pipeline stopped before executing, which happens when a
    /// pre-request script asked to skip the request or when a credential is
    /// awaiting re-entry.
    pub execution: Option<Execution>,
    /// Timeline entries produced outside the engine, from scripts and plugins.
    pub entries: Vec<TimelineEntry>,
    /// The stages that ran, in order, for the UI's detail view.
    pub stages: Vec<PlanStage>,
    /// The request's documents were changed by a script and the change is
    /// waiting to be persisted.
    pub patches: Vec<heric_model::Patch>,
    /// Why the send stopped before executing, when it did.
    pub stopped: Option<StopReason>,
    /// Assertion results from the after-response script.
    pub tests: Vec<heric_script::TestResult>,
}

impl SendOutcome {
    /// Whether the request produced a response.
    #[must_use]
    pub fn produced_response(&self) -> bool {
        self.execution.is_some()
    }

    /// The status code, when there is one.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.execution.as_ref().map(|execution| execution.status)
    }
}

/// Why a send stopped early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// A pre-request script called `insomnia.execution.skipRequest`.
    SkippedByScript,
    /// A credential must be entered again before the request can go.
    PendingCredential {
        /// What to ask for.
        field: String,
    },
    /// The user cancelled before the request was executed.
    Cancelled,
    /// A pre-request script threw and the user's settings stop the send.
    ///
    /// The original sends anyway by default: a broken script logs to the timeline
    /// and the request still goes out.
    ScriptFailed,
}

/// A send, broken into its stages.
///
/// Built by the request service before the send starts so the UI can show the
/// plan and the pipeline can report progress against it.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// The request being sent.
    pub request: Id,
    /// Which run.
    pub run: u64,
    /// The stages this send will attempt.
    ///
    /// Shorter than [`PlanStage::ORDER`] when the request has no script or no
    /// authentication, which is common and makes the progress display honest.
    pub stages: Vec<PlanStage>,
    /// The cancellation token.
    pub cancel: heric_http::CancelToken,
}

impl ExecutionPlan {
    /// A plan for a request with no scripts and no auth.
    #[must_use]
    pub fn minimal(request: Id, run: u64) -> Self {
        Self {
            request,
            run,
            stages: vec![
                PlanStage::Inherit,
                PlanStage::Render,
                PlanStage::BuildBody,
                PlanStage::Execute,
                PlanStage::StoreResponse,
                PlanStage::RecordHistory,
            ],
            cancel: heric_http::CancelToken::new(),
        }
    }

    /// Whether a stage will run.
    #[must_use]
    pub fn includes(&self, stage: PlanStage) -> bool {
        self.stages.contains(&stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canonical_order_has_eleven_stages() {
        assert_eq!(PlanStage::ORDER.len(), 11);
        assert_eq!(PlanStage::ORDER[0], PlanStage::Inherit);
        assert_eq!(PlanStage::ORDER[10], PlanStage::RecordHistory);
        assert_eq!(
            PlanStage::ORDER[6],
            PlanStage::Execute,
            "executing must follow every preparation stage"
        );
    }

    #[test]
    fn a_minimal_plan_skips_the_script_stages() {
        let plan = ExecutionPlan::minimal(Id::new("req_1"), 1);
        assert!(!plan.includes(PlanStage::PreRequestScript));
        assert!(!plan.includes(PlanStage::Authenticate));
        assert!(plan.includes(PlanStage::Execute));
    }

    #[test]
    fn an_outcome_without_an_execution_has_no_status() {
        let outcome = SendOutcome::default();
        assert!(!outcome.produced_response());
        assert_eq!(outcome.status(), None);
    }

    #[test]
    fn an_outcome_with_an_execution_reports_its_status() {
        let outcome = SendOutcome {
            execution: Some(Execution::ok(201)),
            ..SendOutcome::default()
        };
        assert!(outcome.produced_response());
        assert_eq!(outcome.status(), Some(201));
    }
}
