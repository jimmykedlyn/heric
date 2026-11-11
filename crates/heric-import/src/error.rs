//! Errors from import and export.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Why a source could not be imported, or a document exported.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// No format matched, or the format is not one Heric reads.
    #[error("unrecognised file: {0}")]
    UnrecognisedFormat(String),

    /// The source does not parse as the format it claims.
    #[error("{format} does not parse: {detail}")]
    Parse {
        /// The format.
        format: &'static str,
        /// Where and why.
        detail: String,
    },

    /// A required field is missing.
    #[error("{format} is missing {field}")]
    Missing {
        /// The format.
        format: &'static str,
        /// The field.
        field: String,
    },

    /// The source is a newer version than this build reads.
    #[error("{0} is newer than this build supports")]
    TooNew(String),

    /// A legacy format did not match strictly, so it was refused rather than
    /// guessed at.
    #[error("not a {0} export: the format marker does not match")]
    MarkerMismatch(&'static str),

    /// The archive could not be read.
    #[error("cannot read the archive: {0}")]
    Archive(String),

    /// Nothing importable was found.
    #[error("the file contains no requests")]
    Empty,

    /// An export target does not support something the collection contains.
    #[error("cannot export {feature} to {format}")]
    NotExportable {
        /// The feature.
        feature: &'static str,
        /// The target format.
        format: &'static str,
    },
}

impl Diagnostic for ImportError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::UnrecognisedFormat(_) => MessageKey::new("heric-import.unrecognised-format"),
            Self::Parse { .. } => MessageKey::new("heric-import.parse"),
            Self::Missing { .. } => MessageKey::new("heric-import.missing-field"),
            Self::TooNew(_) => MessageKey::new("heric-import.too-new"),
            Self::MarkerMismatch(_) => MessageKey::new("heric-import.marker-mismatch"),
            Self::Archive(_) => MessageKey::new("heric-import.archive"),
            Self::Empty => MessageKey::new("heric-import.empty"),
            Self::NotExportable { .. } => MessageKey::new("heric-import.not-exportable"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::UnrecognisedFormat(detail) | Self::Archive(detail) => {
                vec![("detail".to_owned(), detail.as_str().into())]
            }
            Self::Parse { format, detail } => vec![
                ("format".to_owned(), (*format).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Missing { format, field } => vec![
                ("format".to_owned(), (*format).into()),
                ("field".to_owned(), field.as_str().into()),
            ],
            Self::TooNew(version) => vec![("version".to_owned(), version.as_str().into())],
            Self::MarkerMismatch(format) => vec![("format".to_owned(), (*format).into())],
            Self::NotExportable { feature, format } => vec![
                ("feature".to_owned(), (*feature).into()),
                ("format".to_owned(), (*format).into()),
            ],
            Self::Empty => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::TooNew(_) => Some(MessageKey::new("heric-import.hint.update-app")),
            Self::UnrecognisedFormat(_) => {
                Some(MessageKey::new("heric-import.hint.supported-formats"))
            }
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type ImportDiagnostic = ImportError;
