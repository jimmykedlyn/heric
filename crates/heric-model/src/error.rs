//! Error type for the model crate.

/// Things that can go wrong when reading or interpreting a document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// A `type` field carried a name this build does not know, which is what a
    /// file written by a newer version looks like.
    #[error("unknown document type: {0}")]
    UnknownKind(String),

    /// A typed view was requested for a document of a different kind.
    #[error("expected a {expected} document, found {found}")]
    WrongKind {
        /// The kind the view requires.
        expected: crate::DocKind,
        /// The kind the document actually carries.
        found: crate::DocKind,
    },

    /// A required field was missing or had the wrong JSON type.
    #[error("field `{field}` is missing or invalid")]
    MissingField {
        /// Name of the field.
        field: &'static str,
    },
}
