//! Commits, diffs and sync state.

use std::path::PathBuf;

use crate::BranchName;

/// What to commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitRequest {
    /// The message. Empty means the UI asks for one; the backend never invents
    /// one, because the original's AI-generated messages are opt-in (`IO-027`).
    pub message: String,
    /// Author name, or the repository's configured one when absent.
    pub author: Option<String>,
    /// Author email.
    pub author_email: Option<String>,
    /// When to commit.
    pub when: CommitWhen,
}

/// Which changes to include.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CommitWhen {
    /// Every change in the working tree.
    #[default]
    Everything,
    /// Only the listed paths, which is what a partial stage produces (`IO-025`).
    Only(Vec<PathBuf>),
}

/// One changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path relative to the repository root.
    pub path: String,
    /// What happened to it.
    pub kind: DiffKind,
    /// How many lines were added.
    pub added: usize,
    /// How many were removed.
    pub removed: usize,
}

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// The file is new.
    Added,
    /// The file changed.
    Modified,
    /// The file was removed.
    Deleted,
    /// The file was renamed.
    Renamed,
}

/// One entry in the diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    /// The file.
    pub path: String,
    /// The change.
    pub kind: DiffKind,
    /// The hunks, already rendered, so the widget does no parsing.
    pub hunks: Vec<DiffHunk>,
}

/// One contiguous change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// The header line, `@@ -1,4 +1,6 @@`.
    pub header: String,
    /// The lines, with their leading marker already applied.
    pub lines: Vec<String>,
}

/// A push's result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushOutcome {
    /// Whether anything was actually sent. A push with nothing to send is a
    /// success, and the UI says "already up to date" rather than "pushed".
    pub sent: bool,
    /// How many commits went out.
    pub commits: usize,
    /// The branch pushed.
    pub branch: Option<BranchName>,
}

/// The collection's sync state, as the sidebar shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncStatus {
    /// The current branch, `None` for a detached head or an initialised repo with
    /// no commit yet.
    pub branch: Option<BranchName>,
    /// How many commits behind the remote.
    pub behind: usize,
    /// How many ahead.
    pub ahead: usize,
    /// Whether the working tree has changes.
    pub has_changes: bool,
    /// Whether a merge is in progress.
    pub merging: bool,
}

impl SyncStatus {
    /// Whether the collection is in sync with its remote.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.ahead == 0 && self.behind == 0 && !self.has_changes && !self.merging
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_status_is_clean() {
        assert!(SyncStatus::default().is_clean());
    }

    #[test]
    fn a_change_makes_it_dirty() {
        let status = SyncStatus {
            has_changes: true,
            ..SyncStatus::default()
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn being_behind_counts_as_out_of_sync() {
        let status = SyncStatus {
            behind: 1,
            ..SyncStatus::default()
        };
        assert!(!status.is_clean());
    }

    #[test]
    fn nothing_to_push_is_still_a_success() {
        assert!(!PushOutcome::default().sent);
    }
}
