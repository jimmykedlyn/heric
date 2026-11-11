//! Git sync for collections.
//!
//! Git-backed collections are the migration path for users who already store a
//! collection in a repository. Two layouts exist and both must be read: the
//! legacy `.insomnia/<Type>/<id>.yml` tree and the current root-level
//! `insomnia.<id>.yaml` files, detected per collection rather than per
//! application, because a user can have both (`STO-011`).
//!
//! The backend is `gix`, not `git2`: `libgit2` is GPL-2.0 with a linking
//! exception, and the workspace's licence allow-list blocks GPL (ADR 0007).

#![deny(unsafe_code)]

mod branch;
mod capture;
mod error;
mod layout;
mod repo;
mod sync;

pub use branch::BranchName;
pub use capture::{CommitRequest, DiffEntry, DiffKind, FileChange, PushOutcome, SyncStatus};
pub use error::{GitDiagnostic, GitError};
pub use layout::{CollectionLayout, LayoutFile};
pub use repo::{
    Credentials, RemoteCredentials, Repository, RepositoryHandle, git_directory,
    normalize_remote_url,
};
pub use sync::{Conflict, ConflictResolution, MergeOutcome};

/// Returns the version of `heric-git`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Opens or initialises a repository for a collection.
///
/// # Errors
///
/// Returns [`GitError::Open`] when the directory is not a repository and cannot be
/// initialised, and [`GitError::Blocked`] when the path is inside another
/// repository in a way that would make a commit escape the collection.
pub fn open(path: &std::path::Path) -> Result<RepositoryHandle, GitError> {
    let _ = path;
    todo!("M6: open or initialise the collection's repository")
}

/// Detects which layout a working tree uses.
///
/// # Errors
///
/// Returns [`GitError::Layout`] when both layouts are present in the same tree,
/// which the original refuses rather than guessing.
pub fn detect_layout(root: &std::path::Path) -> Result<CollectionLayout, GitError> {
    let _ = root;
    todo!("M6: detect the collection layout")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
