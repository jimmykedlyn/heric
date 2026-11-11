//! Errors for the storage crate.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Anything that can go wrong in storage.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The platform reported no data directory.
    #[error("no platform data directory is available")]
    NoDataDir,

    /// The source data directory could not be found.
    #[error("no Insomnia data directory found at {0}")]
    SourceNotFound(String),

    /// A source file could not be read.
    #[error("cannot read {path}: {source}")]
    SourceUnreadable {
        /// The file that failed.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// More lines were corrupt than the original's threshold tolerates.
    #[error("{corrupt} of {total} lines in {path} are unreadable")]
    CorruptBeyondThreshold {
        /// The file that failed.
        path: String,
        /// Lines that did not parse.
        corrupt: usize,
        /// Lines read.
        total: usize,
    },

    /// An insert collided with an existing id.
    #[error("a document with id {0} already exists")]
    DuplicateId(String),

    /// A document was not found.
    #[error("no document with id {0}")]
    NotFound(String),

    /// The on-disk schema is newer than this build understands.
    #[error("database schema version {found} is newer than {supported}")]
    SchemaTooNew {
        /// Version found on disk.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },

    /// The database could not be opened or written.
    #[error("database error: {0}")]
    Database(String),

    /// A blob operation failed.
    #[error("blob error: {0}")]
    Blob(String),
}

impl Diagnostic for StorageError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::NoDataDir => MessageKey::new("heric-storage.no-data-dir"),
            Self::SourceNotFound(_) => MessageKey::new("heric-storage.source-not-found"),
            Self::SourceUnreadable { .. } => MessageKey::new("heric-storage.source-unreadable"),
            Self::CorruptBeyondThreshold { .. } => {
                MessageKey::new("heric-storage.corrupt-beyond-threshold")
            }
            Self::DuplicateId(_) => MessageKey::new("heric-storage.duplicate-id"),
            Self::NotFound(_) => MessageKey::new("heric-storage.not-found"),
            Self::SchemaTooNew { .. } => MessageKey::new("heric-storage.schema-too-new"),
            Self::Database(_) => MessageKey::new("heric-storage.database"),
            Self::Blob(_) => MessageKey::new("heric-storage.blob"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::SourceNotFound(path) => vec![("path".to_owned(), path.as_str().into())],
            Self::SourceUnreadable { path, .. } => {
                vec![("path".to_owned(), path.as_str().into())]
            }
            Self::CorruptBeyondThreshold {
                path,
                corrupt,
                total,
            } => vec![
                ("path".to_owned(), path.as_str().into()),
                ("corrupt".to_owned(), (*corrupt as i64).into()),
                ("total".to_owned(), (*total as i64).into()),
            ],
            Self::DuplicateId(id) | Self::NotFound(id) => {
                vec![("id".to_owned(), id.as_str().into())]
            }
            Self::SchemaTooNew { found, supported } => vec![
                ("found".to_owned(), i64::from(*found).into()),
                ("supported".to_owned(), i64::from(*supported).into()),
            ],
            Self::Database(text) | Self::Blob(text) => {
                vec![("detail".to_owned(), text.as_str().into())]
            }
            Self::NoDataDir => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::SourceNotFound(_) => Some(MessageKey::new("heric-storage.hint.set-data-path")),
            Self::SchemaTooNew { .. } => Some(MessageKey::new("heric-storage.hint.update-app")),
            _ => None,
        }
    }
}

/// Alias kept so call sites can be explicit about what they are matching on.
pub type StorageDiagnostic = StorageError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_key() {
        let errors = [
            StorageError::NoDataDir,
            StorageError::SourceNotFound("/tmp".to_owned()),
            StorageError::DuplicateId("req_1".to_owned()),
            StorageError::NotFound("req_1".to_owned()),
            StorageError::SchemaTooNew {
                found: 3,
                supported: 2,
            },
        ];
        for error in &errors {
            assert!(error.message_key().as_str().starts_with("heric-storage."));
        }
    }

    #[test]
    fn arguments_carry_the_path() {
        let error = StorageError::SourceNotFound("/tmp/insomnia".to_owned());
        assert_eq!(
            error.message_args(),
            vec![(
                "path".to_owned(),
                MessageValue::Text("/tmp/insomnia".to_owned())
            )]
        );
    }
}
