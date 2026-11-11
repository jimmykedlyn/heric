//! Merging and conflict resolution.

/// One conflicting file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Path relative to the repository root.
    pub path: String,
    /// Whether the conflict is a whole-file add/add, which the document-level
    /// merge can usually settle without asking.
    pub add_add: bool,
    /// The three sides, when the merge could read them.
    pub sides: Option<String>,
}

/// How the user settled a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Keep the local version.
    Ours,
    /// Keep the remote version.
    Theirs,
    /// Keep both, which only makes sense for a conflict over a list of requests.
    Both,
}

/// What a merge produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeOutcome {
    /// Files that merged without help.
    pub merged: Vec<String>,
    /// Files that need the user.
    pub conflicts: Vec<Conflict>,
}

impl MergeOutcome {
    /// Whether the merge finished cleanly.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }

    /// Applies one resolution to the list.
    ///
    /// A resolution of [`ConflictResolution::Both`] on a file whose conflict is
    /// not a whole-file add/add leaves it in the list, because keeping both sides
    /// of a line-level conflict would produce a document that is not valid.
    pub fn resolve(&mut self, path: &str, resolution: ConflictResolution) {
        if resolution == ConflictResolution::Both
            && let Some(conflict) = self.conflicts.iter().find(|c| c.path == path)
            && !conflict.add_add
        {
            return;
        }
        self.conflicts.retain(|conflict| conflict.path != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome() -> MergeOutcome {
        MergeOutcome {
            merged: vec!["a.yaml".to_owned()],
            conflicts: vec![
                Conflict {
                    path: "b.yaml".to_owned(),
                    add_add: false,
                    sides: None,
                },
                Conflict {
                    path: "c.yaml".to_owned(),
                    add_add: true,
                    sides: None,
                },
            ],
        }
    }

    #[test]
    fn a_conflict_keeps_the_merge_unclean() {
        assert!(!outcome().is_clean());
    }

    #[test]
    fn resolving_removes_only_that_file() {
        let mut merge = outcome();
        merge.resolve("b.yaml", ConflictResolution::Ours);
        assert_eq!(merge.conflicts.len(), 1);
        assert_eq!(merge.conflicts[0].path, "c.yaml");
        assert!(!merge.is_clean());
    }

    #[test]
    fn keeping_both_is_refused_for_a_line_level_conflict() {
        let mut merge = outcome();
        merge.resolve("b.yaml", ConflictResolution::Both);
        assert_eq!(merge.conflicts.len(), 2, "the resolution must not apply");
    }

    #[test]
    fn keeping_both_works_for_an_add_add() {
        let mut merge = outcome();
        merge.resolve("c.yaml", ConflictResolution::Both);
        assert_eq!(merge.conflicts.len(), 1);
    }
}
