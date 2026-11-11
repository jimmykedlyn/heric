//! The repository handle and credentials.

use std::path::{Path, PathBuf};

use crate::{BranchName, CommitRequest, DiffEntry, GitError, PushOutcome, SyncStatus};

/// Where a remote's credentials come from.
///
/// Four providers in the original: GitHub and GitLab OAuth, a custom token, and
/// the operating system's credential helper. The helper is last on purpose: it is
/// the only one that never puts a secret in Heric's own storage (`PLT-006`).
#[derive(Clone, Default, PartialEq, Eq)]
pub enum Credentials {
    /// No credentials; an anonymous fetch or a local remote.
    #[default]
    None,
    /// A token, used as the password with `x-access-token` as the user.
    Token {
        /// The token.
        token: String,
    },
    /// A username and password.
    UserPass {
        /// The user.
        username: String,
        /// The password or token.
        password: String,
    },
    /// Delegate to `git credential fill`, which reads the OS store.
    ///
    /// Never stores anything itself: the values are held only for the duration of
    /// one operation.
    NativeHelper,
}

impl std::fmt::Debug for Credentials {
    /// Redacts the secret, so a debug print in a log line cannot leak it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Token { .. } => f.write_str("Token { token: <redacted> }"),
            Self::UserPass { username, .. } => {
                write!(
                    f,
                    "UserPass {{ username: {username}, password: <redacted> }}"
                )
            }
            Self::NativeHelper => f.write_str("NativeHelper"),
        }
    }
}

/// Which credentials apply to a remote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteCredentials {
    /// The remote's name, `origin` by default.
    pub remote: String,
    /// The credentials.
    pub credentials: Credentials,
}

/// An open repository.
///
/// Every method is blocking. Git work is disk- and sometimes network-bound, so it
/// runs on the blocking pool like the other synchronous ports (ADR 0009).
pub trait Repository: Send + Sync {
    /// The working tree's path.
    fn workdir(&self) -> &Path;

    /// The current sync state.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Backend`] when the tree cannot be read.
    fn status(&self) -> Result<SyncStatus, GitError>;

    /// Stages and commits.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Dirty`] when the request names paths but the tree has
    /// changes outside them that would silently be left behind, and
    /// [`GitError::ConflictMarkers`] when a file about to be committed still
    /// holds merge markers.
    fn commit(&self, request: &CommitRequest) -> Result<String, GitError>;

    /// Fetches from the remote.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Auth`] for a credential failure and
    /// [`GitError::Remote`] for anything else.
    fn fetch(&self, credentials: &RemoteCredentials) -> Result<(), GitError>;

    /// Pushes the current branch.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Auth`] or [`GitError::Remote`].
    fn push(&self, credentials: &RemoteCredentials) -> Result<PushOutcome, GitError>;

    /// The diff between the working tree and the last commit.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Backend`] when a file cannot be read.
    fn diff(&self) -> Result<Vec<DiffEntry>, GitError>;

    /// Creates a branch and checks it out.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Branch`] when the name exists or is invalid, and
    /// [`GitError::Dirty`] when the checkout would lose changes.
    fn create_branch(&self, name: &BranchName) -> Result<(), GitError>;

    /// Checks out an existing branch.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Branch`] when it does not exist.
    fn checkout(&self, name: &BranchName) -> Result<(), GitError>;

    /// Deletes a branch.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Branch`] when it is checked out or unmerged, which the
    /// original also refuses rather than force-deleting.
    fn delete_branch(&self, name: &BranchName) -> Result<(), GitError>;

    /// Lists local branches.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Backend`].
    fn branches(&self) -> Result<Vec<BranchName>, GitError>;

    /// The amount of disk the collection's `.git` directory uses, for the
    /// settings screen.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::Backend`] when the directory cannot be walked.
    fn disk_usage(&self) -> Result<u64, GitError>;
}

/// A shared handle to a repository.
pub type RepositoryHandle = std::sync::Arc<dyn Repository>;

/// A remote's URL, normalised so two spellings of one repository compare equal.
///
/// `git@github.com:user/repo.git`, `https://github.com/user/repo` and
/// `https://github.com/user/repo.git` all name one repository, and the original
/// duplicates a collection when it fails to notice that.
#[must_use]
pub fn normalize_remote_url(url: &str) -> String {
    let mut normalized = url.trim().to_owned();
    if let Some(rest) = normalized.strip_prefix("git@") {
        normalized = rest.replacen(':', "/", 1);
        normalized = format!("https://{normalized}");
    }
    while normalized.ends_with('/') {
        normalized.pop();
    }
    if let Some(rest) = normalized.strip_suffix(".git") {
        normalized = rest.to_owned();
    }
    normalized.to_ascii_lowercase()
}

/// The directory inside the collection that holds repository data.
///
/// Returned rather than hard-coded so the write guard has one place to consult
/// and a symlink check has one path to resolve.
#[must_use]
pub fn git_directory(workdir: &Path) -> PathBuf {
    workdir.join(".git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_never_print_their_secret() {
        let token = Credentials::Token {
            token: "s3cret".to_owned(),
        };
        assert!(!format!("{token:?}").contains("s3cret"));

        let pair = Credentials::UserPass {
            username: "me".to_owned(),
            password: "s3cret".to_owned(),
        };
        let rendered = format!("{pair:?}");
        assert!(!rendered.contains("s3cret"));
        assert!(rendered.contains("me"));
    }

    #[test]
    fn three_spellings_of_one_remote_normalise_together() {
        let ssh = normalize_remote_url("git@github.com:user/repo.git");
        let https = normalize_remote_url("https://github.com/user/repo");
        let with_suffix = normalize_remote_url("https://github.com/user/repo.git/");
        assert_eq!(ssh, https);
        assert_eq!(https, with_suffix);
    }

    #[test]
    fn the_git_directory_is_inside_the_workdir() {
        let path = git_directory(Path::new("/collections/a"));
        assert!(path.starts_with("/collections/a"));
        assert!(path.ends_with(".git"));
    }
}
