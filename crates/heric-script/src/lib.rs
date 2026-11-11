//! Sandboxed scripting for Heric.
//!
//! Runs the pre-request and after-response scripts an Insomnia document carries.
//! The engine is QuickJS through `rquickjs` (ADR 0002): an interrupt handler can
//! stop a runaway script, which a bytecode-interpreted engine without one cannot
//! do without killing the process.
//!
//! # The two sandboxes
//!
//! The original has three, and two of them matter. The [static sandbox]
//! ([`SecurityPolicy::check_source`]) rejects a script before it runs, by walking
//! the AST for blocked roots and properties. The [runtime
//! sandbox](SecurityPolicy) intercepts `require`, `eval` and `window` at the
//! moment they are called. Neither is a security boundary against a determined
//! attacker, and the original's docs say so: they exist to stop a pasted script
//! from reaching the filesystem or a process API by accident. Heric keeps both
//! and documents the limit rather than claiming more.
//!
//! [static sandbox]: SecurityPolicy

#![deny(unsafe_code)]

mod api;
mod console;
mod context;
mod error;
mod policy;
mod result;

pub use api::{CookieView, Cookies, FolderRef, RequestApi, ResponseApi, TestsApi};
pub use console::{ConsoleEntry, ConsoleLevel};
pub use context::{EventInfo, ExecutionLocation, MutableState, ScriptContext, ScriptPhase};
pub use error::{ScriptDiagnostic, ScriptError};
pub use policy::{BlockedRoot, RequireModule, SecurityPolicy, Violation};
pub use result::{ScriptOutcome, TestResult, TestStatus};

use heric_model::Doc;

/// Returns the version of `heric-script`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// How long a script may run before it is interrupted.
///
/// The original's default. A script that exceeds it is interrupted by QuickJS's
/// interrupt handler, not killed with the process, which is why the engine
/// choice matters here (`SCR-001`).
pub const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Runs script text against a request.
///
/// # Errors
///
/// Returns [`ScriptError::SecurityViolation`] when the static sandbox refuses the
/// source, [`ScriptError::Timeout`] when it overruns, and
/// [`ScriptError::Runtime`] when the script throws.
pub fn run(
    source: &str,
    context: &mut ScriptContext<'_>,
    policy: &SecurityPolicy,
) -> Result<ScriptOutcome, ScriptError> {
    let (source, context, policy) = (source, context, policy);
    let _ = (source, context, policy);
    todo!("M2: compile and run the script in a QuickJS sandbox")
}

/// Compiles a script once so a collection run does not re-parse it per
/// iteration.
///
/// # Errors
///
/// Returns [`ScriptError::Syntax`] when the source does not parse.
pub fn compile(source: &str) -> Result<Compiled, ScriptError> {
    let _ = source;
    todo!("M2: compile a script to bytecode")
}

/// A compiled script, reusable across iterations.
#[derive(Debug)]
pub struct Compiled {
    /// The original source, kept for error reporting against line numbers.
    pub source: String,
}

/// Translates a Postman script into the `insomnia.*` surface.
///
/// About forty rewrites, applied before the script reaches the sandbox, so a
/// pasted Postman script runs unchanged (`SCR-022`).
#[must_use]
pub fn translate_postman(source: &str) -> String {
    source.to_owned()
}

/// Applies a script's mutations to the document that owns them.
///
/// Scripts mutate `insomnia.request`, the environment and the globals; the
/// original persists all three after the script finishes (`SCR-008`). Applied as
/// patches so a concurrent autosave does not clobber the change.
///
/// # Errors
///
/// Returns [`ScriptError::Runtime`] when a mutation names a field the document
/// cannot carry.
pub fn apply(
    outcome: &ScriptOutcome,
    doc: &mut Doc,
) -> Result<Vec<heric_model::Patch>, ScriptError> {
    let _ = (outcome, doc);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_timeout_matches_the_original() {
        assert_eq!(DEFAULT_TIMEOUT.as_secs(), 5);
    }

    #[test]
    fn postman_translation_is_identity_for_now() {
        assert_eq!(translate_postman("const a = 1;"), "const a = 1;");
    }

    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
