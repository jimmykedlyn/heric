//! Errors from snippet generation.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Why a snippet could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    /// No generator handles the pair.
    #[error("no snippet generator for {target} / {client}")]
    UnsupportedTarget {
        /// The target.
        target: &'static str,
        /// The client.
        client: &'static str,
    },

    /// The request uses something the target cannot express.
    #[error("{feature} cannot be expressed in {target}")]
    UnsupportedFeature {
        /// The feature.
        feature: &'static str,
        /// The target.
        target: &'static str,
    },

    /// A value could not be escaped for the target's syntax.
    #[error("cannot escape a value for {target}: {detail}")]
    Escaping {
        /// The target.
        target: &'static str,
        /// Why.
        detail: String,
    },

    /// A body file could not be read.
    #[error("cannot read the request body: {0}")]
    Body(String),
}

impl Diagnostic for CodegenError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::UnsupportedTarget { .. } => MessageKey::new("heric-codegen.unsupported-target"),
            Self::UnsupportedFeature { .. } => MessageKey::new("heric-codegen.unsupported-feature"),
            Self::Escaping { .. } => MessageKey::new("heric-codegen.escaping"),
            Self::Body(_) => MessageKey::new("heric-codegen.body"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::UnsupportedTarget { target, client } => vec![
                ("target".to_owned(), (*target).into()),
                ("client".to_owned(), (*client).into()),
            ],
            Self::UnsupportedFeature { feature, target } => vec![
                ("feature".to_owned(), (*feature).into()),
                ("target".to_owned(), (*target).into()),
            ],
            Self::Escaping { target, detail } => vec![
                ("target".to_owned(), (*target).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Body(detail) => vec![("detail".to_owned(), detail.as_str().into())],
        }
    }
}

/// Alias for symmetry with the other crates.
pub type CodegenDiagnostic = CodegenError;
