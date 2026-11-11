//! Branches.

/// A branch name that has been validated.
///
/// A newtype rather than a `String` because a branch name reaches `git` as a
/// path component: `..`, a leading `/`, a trailing `.lock` and a `~^:?*[` all
/// produce either a broken reference or, in the worst case, a write outside the
/// repository. Validating at construction means no lower layer has to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchName(String);

impl BranchName {
    /// Validates a branch name.
    ///
    /// # Errors
    ///
    /// Returns [`crate::GitError::Branch`] with `action` set to `"create"` when
    /// the name is not usable.
    pub fn new(raw: impl Into<String>) -> Result<Self, crate::GitError> {
        let raw = raw.into();
        let action = "create";
        if raw.is_empty() {
            return Err(crate::GitError::Branch {
                action,
                name: raw,
                detail: "the name is empty".to_owned(),
            });
        }
        if raw.starts_with('-') || raw.starts_with('/') || raw.ends_with('/') {
            return Err(crate::GitError::Branch {
                action,
                name: raw,
                detail: "a name cannot start with `-` or `/`, or end with `/`".to_owned(),
            });
        }
        if raw.contains("..") {
            return Err(crate::GitError::Branch {
                action,
                name: raw,
                detail: "a name cannot contain `..`".to_owned(),
            });
        }
        if raw.contains(|c: char| c.is_whitespace() || "~^:?*[\\".contains(c)) {
            return Err(crate::GitError::Branch {
                action,
                name: raw,
                detail: "a name cannot contain whitespace or ~ ^ : ? * [ \\".to_owned(),
            });
        }
        if raw.ends_with(".lock") {
            return Err(crate::GitError::Branch {
                action,
                name: raw,
                detail: "a name cannot end with `.lock`".to_owned(),
            });
        }
        Ok(Self(raw))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BranchName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(BranchName::new("feature/sync")?.as_str(), "feature/sync");
        Ok(())
    }

    #[test]
    fn a_traversal_is_rejected() {
        assert!(BranchName::new("a..b").is_err());
        assert!(BranchName::new("../escape").is_err());
    }

    #[test]
    fn a_leading_dash_is_rejected_because_it_looks_like_a_flag() {
        assert!(BranchName::new("--force").is_err());
    }

    #[test]
    fn a_lock_suffix_is_rejected() {
        assert!(BranchName::new("main.lock").is_err());
    }

    #[test]
    fn empty_is_rejected() {
        assert!(BranchName::new("").is_err());
    }
}
