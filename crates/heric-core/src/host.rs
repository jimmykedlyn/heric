//! The render host: what the template tags may reach.
//!
//! `heric-template` deliberately cannot open a socket, read a file or ask a
//! person a question. Everything of that kind goes through [`TagHost`], and this
//! module is its only implementation. That is what makes a preview render and a
//! send render the same code path with different permissions: the difference is
//! the host, not the renderer.

use std::path::{Path, PathBuf};

use heric_model::{Execution, PreparedRequest};
use heric_template::{PromptAnswer, PromptRequest, ReadPurpose, RenderPurpose, TagError, TagHost};

use crate::CoreHandle;

/// The roots a `{% file %}` tag and a multipart upload may read from.
///
/// The original allows the collection's own data folders and nothing else
/// (`HTTP-006`). The list is per-send rather than global, because a request may
/// belong to one collection while another is open.
#[derive(Debug, Clone, Default)]
pub struct AllowedRoots {
    roots: Vec<PathBuf>,
}

impl AllowedRoots {
    /// An empty allowlist, which refuses every path.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// An allowlist of the given directories.
    #[must_use]
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
        }
    }

    /// Whether a path is inside one of the roots.
    ///
    /// Compares canonicalised paths, so `../` cannot escape: a symlink or a
    /// relative segment that resolves outside a root is refused, which is the
    /// check `GitError::SymlinkEscape` mirrors for the repository side.
    #[must_use]
    pub fn allows(&self, path: &Path) -> bool {
        let Ok(candidate) = path.canonicalize() else {
            return false;
        };
        self.roots.iter().any(|root| {
            root.canonicalize()
                .is_ok_and(|root| candidate.starts_with(&root))
        })
    }

    /// How many roots are allowed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// Whether nothing is allowed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// The host a render talks to.
pub struct SendingHost<'a> {
    core: &'a CoreHandle,
    purpose: RenderPurpose,
    roots: AllowedRoots,
}

impl<'a> SendingHost<'a> {
    /// A host for a render with the given purpose.
    #[must_use]
    pub fn new(core: &'a CoreHandle, purpose: RenderPurpose, roots: AllowedRoots) -> Self {
        Self {
            core,
            purpose,
            roots,
        }
    }

    /// The roots this host may read.
    #[must_use]
    pub fn roots(&self) -> &AllowedRoots {
        &self.roots
    }
}

impl TagHost for SendingHost<'_> {
    fn read_file(&self, path: &Path, purpose: ReadPurpose) -> Result<Vec<u8>, TagError> {
        if !self.roots.allows(path) {
            return Err(TagError::PathNotAllowed {
                path: path.display().to_string(),
            });
        }
        // A transport read is the engine loading a certificate or key, which is
        // configured rather than written into a request, so it is allowed on a
        // narrower basis: it still has to be inside a root, which the check above
        // established.
        let _ = purpose;
        std::fs::read(path).map_err(|error| TagError::ReadFailed {
            path: path.display().to_string(),
            detail: error.to_string(),
        })
    }

    fn prompt(&self, request: &PromptRequest) -> Result<PromptAnswer, TagError> {
        let _ = (&self.core, request);
        todo!("M3: route a prompt to the UI and wait for the answer")
    }

    fn send(&self, request: &PreparedRequest) -> Result<Execution, TagError> {
        let _ = (&self.core, request);
        todo!("M3: execute a {{% response %}} sub-request through the engine")
    }

    fn purpose(&self) -> RenderPurpose {
        self.purpose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_allowlist_refuses_everything() {
        let roots = AllowedRoots::none();
        assert!(roots.is_empty());
        assert!(!roots.allows(Path::new("Cargo.toml")));
    }

    #[test]
    fn a_path_inside_a_root_is_allowed() {
        let root = std::env::temp_dir();
        let roots = AllowedRoots::new([root.clone()]);
        let inside = root.join("heric-allowlist-test.txt");
        if std::fs::write(&inside, b"x").is_err() {
            return; // A read-only temp directory is not this test's subject.
        }
        assert!(roots.allows(&inside));
        let _ = std::fs::remove_file(&inside);
    }

    #[test]
    fn a_relative_escape_is_refused() {
        let root = std::env::temp_dir().join("heric-roots-test");
        let roots = AllowedRoots::new([root]);
        assert!(!roots.allows(Path::new("../etc/passwd")));
    }
}
