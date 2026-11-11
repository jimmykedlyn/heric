//! Errors for the template crate.

use heric_model::{Diagnostic, MessageKey, MessageValue};

use crate::TagError;

/// What kind of failure occurred.
///
/// The original's UI branches on the difference between "undefined variable"
/// and "evaluation error": the first offers to create the variable, the second
/// shows a stack. Collapsing them into one variant would lose that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderErrorKind {
    /// The template did not parse.
    Syntax,
    /// A variable the context does not define was referenced.
    UndefinedVariable,
    /// A filter, tag or operation failed at evaluation time.
    Evaluation,
    /// A tag needed something the host refused.
    Tag,
    /// A limit was exceeded: render steps, output size or memory.
    Limit,
}

/// A rendering failure.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The template did not parse.
    #[error("{path}:{line}:{column}: {message}")]
    Syntax {
        /// Template path, or the request field for an inline template.
        path: String,
        /// Line, 1-based.
        line: u32,
        /// Column, 1-based.
        column: u32,
        /// The parser's message.
        message: String,
    },

    /// A variable was not defined, with `strictVariables` on.
    #[error("{path}:{line}:{column}: undefined variable `{name}`")]
    UndefinedVariable {
        /// Template path.
        path: String,
        /// Line, 1-based.
        line: u32,
        /// Column, 1-based.
        column: u32,
        /// The variable's name.
        name: String,
    },

    /// An operation failed while evaluating.
    #[error("{path}:{line}:{column}: {message}")]
    Evaluation {
        /// Template path.
        path: String,
        /// Line, 1-based.
        line: u32,
        /// Column, 1-based.
        column: u32,
        /// What failed.
        message: String,
    },

    /// A limit was exceeded.
    #[error("render limit exceeded: {what}")]
    Limit {
        /// Which limit.
        what: &'static str,
    },

    /// A tag failed.
    #[error("template tag failed: {0}")]
    Tag(#[from] TagError),
}

impl RenderError {
    /// Which kind of failure this is.
    #[must_use]
    pub fn kind(&self) -> RenderErrorKind {
        match self {
            Self::Syntax { .. } => RenderErrorKind::Syntax,
            Self::UndefinedVariable { .. } => RenderErrorKind::UndefinedVariable,
            Self::Evaluation { .. } => RenderErrorKind::Evaluation,
            Self::Limit { .. } => RenderErrorKind::Limit,
            Self::Tag(_) => RenderErrorKind::Tag,
        }
    }

    /// The template path, when the failure has one.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Syntax { path, .. }
            | Self::UndefinedVariable { path, .. }
            | Self::Evaluation { path, .. } => Some(path),
            Self::Limit { .. } | Self::Tag(_) => None,
        }
    }
}

impl Diagnostic for RenderError {
    fn message_key(&self) -> MessageKey {
        match self.kind() {
            RenderErrorKind::Syntax => MessageKey::new("heric-template.syntax"),
            RenderErrorKind::UndefinedVariable => {
                MessageKey::new("heric-template.undefined-variable")
            }
            RenderErrorKind::Evaluation => MessageKey::new("heric-template.evaluation"),
            RenderErrorKind::Tag => MessageKey::new("heric-template.tag"),
            RenderErrorKind::Limit => MessageKey::new("heric-template.limit"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Syntax {
                path,
                line,
                column,
                message,
            }
            | Self::Evaluation {
                path,
                line,
                column,
                message,
            } => vec![
                ("path".to_owned(), path.as_str().into()),
                ("line".to_owned(), i64::from(*line).into()),
                ("column".to_owned(), i64::from(*column).into()),
                ("message".to_owned(), message.as_str().into()),
            ],
            Self::UndefinedVariable {
                path,
                line,
                column,
                name,
            } => vec![
                ("path".to_owned(), path.as_str().into()),
                ("line".to_owned(), i64::from(*line).into()),
                ("column".to_owned(), i64::from(*column).into()),
                ("name".to_owned(), name.as_str().into()),
            ],
            Self::Limit { what } => vec![("what".to_owned(), (*what).into())],
            Self::Tag(error) => error.message_args(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self.kind() {
            RenderErrorKind::UndefinedVariable => {
                Some(MessageKey::new("heric-template.hint.define-variable"))
            }
            _ => None,
        }
    }
}
