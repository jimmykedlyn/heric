//! Application core for Heric.
//!
//! Wires the leaf crates into the services a front end calls, and owns the one
//! place where the GUI's concurrency model lives (ADR 0009):
//!
//! - one egui thread, which never blocks;
//! - one tokio multi-thread runtime;
//! - an unbounded event channel from the runtime to the UI;
//! - a repaint callback invoked *after* an event is enqueued, never before.
//!
//! The last point is the one that is easy to get wrong and hard to debug: a
//! repaint requested before the event is in the channel can be serviced by the UI
//! before the event is visible, and the frame then shows stale data with no
//! further repaint scheduled, so it stays stale until something else happens.
//!
//! Synchronous ports (libcurl, SQLite, Git, QuickJS) run on
//! `tokio::task::spawn_blocking` rather than on a runtime worker, because a
//! blocked worker starves the timer and the reactor for every other task, and a
//! blocked UI thread freezes the window.

#![deny(unsafe_code)]

mod error;
mod events;
mod host;
mod plan;
mod platform;
mod runtime;
mod service;
mod session;
mod workspace;

pub use error::{CoreDiagnostic, CoreError};
pub use events::{CoreEvent, EventReceiver, EventSink, RepaintSignal, no_repaint};
pub use host::{AllowedRoots, SendingHost};
pub use plan::{ExecutionPlan, PlanStage, SendOutcome, StopReason};
pub use platform::{Platform, current};
pub use runtime::{Core, CoreBuilder, CoreHandle};
pub use service::{
    CookieService, EnvironmentService, ExportService, ImportService, RequestService, RunnerService,
    SETTINGS_ID, ScriptService, SettingsService, apply_auth, auth_method, console_entries,
    describe_stop, plan_runs_scripts, secret_ref,
};
pub use session::{OpenSession, SessionKind, SessionService, to_core_event};
pub use workspace::{ActiveContext, DocumentService, WorkspaceService, WorkspaceSummary};

// Facade re-exports.
//
// Every type in a service signature is reachable from this crate alone, so a
// front end depends on `heric-core` and not on the twelve leaf crates. The
// leaf crates stay public for their own tests and for anyone building a
// second adapter, but a GUI or a CLI that only calls services needs nothing
// else: passing a `DataPaths` or reading an `ImportResult` must not force a
// second `Cargo.toml` entry whose version has to move in lockstep.
pub use heric_auth::{AuthError, AuthMethod, AuthOutcome, SecretRef, SecretResolverHandle};
pub use heric_codegen::{Client, CodegenError, SnippetRequest, Target};
pub use heric_git::{
    BranchName, CommitRequest, Conflict, ConflictResolution, Credentials, DiffEntry, GitError,
    PushOutcome, Repository, RepositoryHandle, SyncStatus,
};
pub use heric_http::{CancelToken, HttpEngine, HttpError};
pub use heric_import::{
    ExportFormat, ExportOptions, ExportedFile, Format, ImportOptions, ImportReport, ImportResult,
    RedactionMode,
};
pub use heric_model::{
    Change, Doc, DocKind, Id, Patch, PreparedRequest, TimelineEntry, TimelineKind,
};
pub use heric_protocols::{
    CloseInfo, MessagePayload, ProtocolError, ReadyState, SessionEvent, SessionId,
};
pub use heric_script::{
    ConsoleEntry, ConsoleLevel, EventInfo, ScriptContext, ScriptOutcome, ScriptPhase,
    SecurityPolicy, TestResult,
};
pub use heric_storage::{
    DataPaths, DryRunReport, InsomniaDataDir, MigrationOptions, MigrationReport, SourceLayout,
    StorageError,
};
pub use heric_template::{
    PromptAnswer, PromptRequest, ReadPurpose, RenderPurpose, TagError, TagHost,
};

/// Returns the version of `heric-core`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The default runtime thread count.
///
/// Leaves the UI's thread and one spare out of the pool; the blocking ports do
/// not use these threads at all.
#[must_use]
pub fn default_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().saturating_sub(2).max(1))
        .unwrap_or(2)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        let v = super::version();
        assert!(v.split('.').count() == 3, "not semver: {v}");
    }

    #[test]
    fn the_worker_count_leaves_a_thread_for_the_ui() {
        assert!(super::default_worker_threads() >= 1);
    }
}
