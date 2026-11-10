//! Git sync for Heric.
//!
//! Implements collection Git sync (clone, pull, push, branch) using the
//! backend chosen in the Phase 1 ADR (gix vs git2, with license
//! implications: libgit2 is GPL-2.0 with a linking exception). See
//! `docs/inventory/io-and-cli.md` once the inventory lands.

/// Error type for Git operations.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// The repository could not be opened.
    #[error("cannot open repository at {0}")]
    OpenFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_failed_message_includes_path() {
        let err = GitError::OpenFailed("/tmp/repo".to_string());
        assert_eq!(err.to_string(), "cannot open repository at /tmp/repo");
    }
}
