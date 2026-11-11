//! Errors from Git operations.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Something Git could not do.
///
/// The distinction that matters throughout: a *remote auth* failure must not be
/// shown as "cannot push", because the user's next action is different. The
/// original maps 401 and 403 into distinct auth errors and Heric keeps that.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// The repository could not be opened.
    #[error("cannot open repository at {0}")]
    Open(String),

    /// The repository is not one, and was not asked to be created.
    #[error("not a git repository: {0}")]
    NotARepository(String),

    /// The remote rejected the credentials.
    #[error("authentication failed for {remote}")]
    Auth {
        /// The remote's name or URL.
        remote: String,
    },

    /// The remote does not exist or cannot be reached.
    #[error("cannot reach {remote}: {detail}")]
    Remote {
        /// The remote's name or URL.
        remote: String,
        /// Why.
        detail: String,
    },

    /// A merge produced conflicts the user must resolve.
    #[error("{0} conflicting files")]
    Conflicts(usize),

    /// Uncommitted changes block the operation.
    #[error("uncommitted changes block this operation")]
    Dirty,

    /// The operation would write outside the collection's directory.
    #[error("refusing to write outside the collection: {0}")]
    Blocked(String),

    /// A symlink escape was detected.
    #[error("refusing to follow a symlink out of the collection: {0}")]
    SymlinkEscape(String),

    /// A file in the tree does not match the collection schema.
    #[error("{path} is not a valid collection file: {detail}")]
    Schema {
        /// The path, relative to the repository.
        path: String,
        /// Why.
        detail: String,
    },

    /// The tree uses both layouts at once.
    #[error("the collection mixes the legacy and current layouts")]
    Layout,

    /// A merge conflict marker was found in a file about to be written.
    ///
    /// The original skips such a file instead of committing the markers.
    #[error("{0} contains merge conflict markers")]
    ConflictMarkers(String),

    /// A branch operation failed.
    #[error("cannot {action} branch {name}: {detail}")]
    Branch {
        /// What was attempted.
        action: &'static str,
        /// The branch.
        name: String,
        /// Why.
        detail: String,
    },

    /// The backend failed.
    #[error("git backend error: {0}")]
    Backend(String),
}

impl Diagnostic for GitError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Open(_) => MessageKey::new("heric-git.open"),
            Self::NotARepository(_) => MessageKey::new("heric-git.not-a-repository"),
            Self::Auth { .. } => MessageKey::new("heric-git.auth"),
            Self::Remote { .. } => MessageKey::new("heric-git.remote"),
            Self::Conflicts(_) => MessageKey::new("heric-git.conflicts"),
            Self::Dirty => MessageKey::new("heric-git.dirty"),
            Self::Blocked(_) => MessageKey::new("heric-git.blocked"),
            Self::SymlinkEscape(_) => MessageKey::new("heric-git.symlink-escape"),
            Self::Schema { .. } => MessageKey::new("heric-git.schema"),
            Self::Layout => MessageKey::new("heric-git.layout"),
            Self::ConflictMarkers(_) => MessageKey::new("heric-git.conflict-markers"),
            Self::Branch { .. } => MessageKey::new("heric-git.branch"),
            Self::Backend(_) => MessageKey::new("heric-git.backend"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Open(detail)
            | Self::NotARepository(detail)
            | Self::Blocked(detail)
            | Self::SymlinkEscape(detail)
            | Self::ConflictMarkers(detail)
            | Self::Backend(detail) => vec![("detail".to_owned(), detail.as_str().into())],
            Self::Auth { remote } => vec![("remote".to_owned(), remote.as_str().into())],
            Self::Remote { remote, detail } => vec![
                ("remote".to_owned(), remote.as_str().into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Conflicts(count) => vec![(
                "count".to_owned(),
                i64::try_from(*count).unwrap_or(i64::MAX).into(),
            )],
            Self::Schema { path, detail } => vec![
                ("path".to_owned(), path.as_str().into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Branch {
                action,
                name,
                detail,
            } => vec![
                ("action".to_owned(), (*action).into()),
                ("name".to_owned(), name.as_str().into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Dirty | Self::Layout => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::Auth { .. } => Some(MessageKey::new("heric-git.hint.check-credentials")),
            Self::Dirty => Some(MessageKey::new("heric-git.hint.commit-or-stash")),
            Self::Conflicts(_) => Some(MessageKey::new("heric-git.hint.resolve-conflicts")),
            Self::Layout => Some(MessageKey::new("heric-git.hint.pick-one-layout")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type GitDiagnostic = GitError;
