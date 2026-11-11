//! Errors from parsing or converting a specification.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Why a specification could not be used.
#[derive(Debug, thiserror::Error)]
pub enum OpenApiError {
    /// The document is neither valid YAML nor valid JSON.
    #[error("the document does not parse: {0}")]
    Syntax(String),

    /// The version is absent or not one Heric reads.
    #[error("unsupported specification version: {0}")]
    UnsupportedVersion(String),

    /// A required field is missing.
    #[error("{field} is required")]
    Missing {
        /// Which field.
        field: &'static str,
    },

    /// A `$ref` points at nothing, or at a component outside `components`.
    #[error("cannot resolve {0}")]
    Unresolved(String),

    /// A remote reference was found. Fetching it would make import depend on the
    /// network, which the original also avoids.
    #[error("remote reference not fetched: {0}")]
    RemoteReference(String),

    /// The document is valid but has nothing to import.
    #[error("the specification has no operations")]
    Empty,

    /// Linting failed hard, as opposed to reporting violations.
    #[error("lint failed: {0}")]
    Lint(String),
}

impl Diagnostic for OpenApiError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Syntax(_) => MessageKey::new("heric-openapi.syntax"),
            Self::UnsupportedVersion(_) => MessageKey::new("heric-openapi.unsupported-version"),
            Self::Missing { .. } => MessageKey::new("heric-openapi.missing-field"),
            Self::Unresolved(_) => MessageKey::new("heric-openapi.unresolved"),
            Self::RemoteReference(_) => MessageKey::new("heric-openapi.remote-reference"),
            Self::Empty => MessageKey::new("heric-openapi.empty"),
            Self::Lint(_) => MessageKey::new("heric-openapi.lint"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Syntax(detail)
            | Self::UnsupportedVersion(detail)
            | Self::Unresolved(detail)
            | Self::RemoteReference(detail)
            | Self::Lint(detail) => vec![("detail".to_owned(), detail.as_str().into())],
            Self::Missing { field } => vec![("field".to_owned(), (*field).into())],
            Self::Empty => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::UnsupportedVersion(_) => {
                Some(MessageKey::new("heric-openapi.hint.convert-to-openapi-3"))
            }
            Self::RemoteReference(_) => {
                Some(MessageKey::new("heric-openapi.hint.inline-the-reference"))
            }
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type OpenApiDiagnostic = OpenApiError;
