//! Errors raised while running a script.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Anything that stops a script from producing a result.
///
/// A script that throws is *not* a failed request in the original: it is
/// reported in the timeline and the request still goes out, because a broken
/// after-response script must not hide the response the user asked for. That is
/// why the caller keeps the two apart: this error means "no result from the
/// script", not "the exchange failed".
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    /// The script did not parse.
    #[error("syntax error at line {line}: {detail}")]
    Syntax {
        /// 1-based line number.
        line: u32,
        /// What the parser said.
        detail: String,
    },

    /// The static sandbox refused the source.
    #[error("script uses a blocked {kind}: {name}")]
    SecurityViolation {
        /// What was blocked.
        kind: &'static str,
        /// Its name.
        name: String,
        /// 1-based line number, when the walk could place it.
        line: Option<u32>,
    },

    /// The script exceeded its budget and was interrupted.
    #[error("script timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// The script threw.
    #[error("{name}: {message}")]
    Runtime {
        /// The error's name, `Error` when it had none.
        name: String,
        /// The message.
        message: String,
        /// The stack, when the engine produced one.
        stack: Option<String>,
    },

    /// A script asked for a module the allowlist does not carry.
    #[error("module `{0}` is not available in scripts")]
    ModuleNotAllowed(String),

    /// A callback-style sub-request never called back, or called back twice.
    #[error("sendRequest callback was {0}")]
    Callback(&'static str),

    /// The script's mutations could not be applied.
    #[error("cannot apply script change to {field}: {detail}")]
    Mutation {
        /// The field.
        field: String,
        /// Why.
        detail: String,
    },

    /// The engine itself failed, which is a bug rather than a script problem.
    #[error("script engine failure: {0}")]
    Engine(String),
}

impl Diagnostic for ScriptError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Syntax { .. } => MessageKey::new("heric-script.syntax"),
            Self::SecurityViolation { .. } => MessageKey::new("heric-script.security-violation"),
            Self::Timeout(_) => MessageKey::new("heric-script.timeout"),
            Self::Runtime { .. } => MessageKey::new("heric-script.runtime"),
            Self::ModuleNotAllowed(_) => MessageKey::new("heric-script.module-not-allowed"),
            Self::Callback(_) => MessageKey::new("heric-script.callback"),
            Self::Mutation { .. } => MessageKey::new("heric-script.mutation"),
            Self::Engine(_) => MessageKey::new("heric-script.engine"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Syntax { line, detail } => vec![
                ("line".to_owned(), i64::from(*line).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::SecurityViolation { kind, name, line } => {
                let mut args = vec![
                    ("kind".to_owned(), (*kind).into()),
                    ("name".to_owned(), name.as_str().into()),
                ];
                if let Some(line) = line {
                    args.push(("line".to_owned(), i64::from(*line).into()));
                }
                args
            }
            Self::Timeout(waited) => {
                vec![("seconds".to_owned(), (waited.as_millis() as i64).into())]
            }
            Self::Runtime { name, message, .. } => vec![
                ("name".to_owned(), name.as_str().into()),
                ("message".to_owned(), message.as_str().into()),
            ],
            Self::ModuleNotAllowed(name) => vec![("name".to_owned(), name.as_str().into())],
            Self::Callback(state) => vec![("state".to_owned(), (*state).into())],
            Self::Mutation { field, detail } => vec![
                ("field".to_owned(), field.as_str().into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Engine(detail) => vec![("detail".to_owned(), detail.as_str().into())],
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::SecurityViolation { .. } => Some(MessageKey::new(
                "heric-script.hint.review-security-settings",
            )),
            Self::Timeout(_) => Some(MessageKey::new("heric-script.hint.increase-timeout")),
            Self::ModuleNotAllowed(_) => Some(MessageKey::new("heric-script.hint.allowed-modules")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type ScriptDiagnostic = ScriptError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_reports_milliseconds() {
        let error = ScriptError::Timeout(std::time::Duration::from_millis(1500));
        assert_eq!(error.to_string(), "script timed out after 1.5s");
        let args = error.message_args();
        assert_eq!(args[0].1, MessageValue::Integer(1500));
    }

    #[test]
    fn a_violation_points_at_the_hint() {
        let error = ScriptError::SecurityViolation {
            kind: "property",
            name: "process.env".to_owned(),
            line: Some(3),
        };
        assert!(error.hint_key().is_some());
        assert_eq!(error.message_args().len(), 3);
    }
}
