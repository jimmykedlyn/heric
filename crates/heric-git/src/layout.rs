//! The two collection layouts a repository can use.

/// Which on-disk layout a collection's working tree uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionLayout {
    /// `.insomnia/<Type>/<id>.yml`, the layout older collections still use.
    LegacyDirectory,
    /// `insomnia.<id>.yaml` at the root, the current layout.
    RootFiles,
}

impl CollectionLayout {
    /// A human-readable name, for the sync dialog.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyDirectory => "legacy .insomnia directory",
            Self::RootFiles => "collection root files",
        }
    }

    /// The directory the layout keeps its files in, relative to the repository
    /// root.
    #[must_use]
    pub fn base_directory(self) -> Option<&'static str> {
        match self {
            Self::LegacyDirectory => Some(".insomnia"),
            Self::RootFiles => None,
        }
    }
}

/// One file in a collection's tree.
///
/// Carries the document kind because the layout decides it: in the root-file
/// layout the kind is inside the YAML, and in the legacy layout it is the parent
/// directory's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFile {
    /// The path relative to the repository root, with forward slashes.
    pub path: String,
    /// The document kind, when the path makes it known.
    pub kind: Option<heric_model::DocKind>,
    /// The document id, when the path makes it known.
    pub id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_legacy_layout_has_a_base_directory() {
        assert_eq!(
            CollectionLayout::LegacyDirectory.base_directory(),
            Some(".insomnia")
        );
        assert_eq!(CollectionLayout::RootFiles.base_directory(), None);
    }

    #[test]
    fn layouts_have_distinct_names() {
        assert_ne!(
            CollectionLayout::LegacyDirectory.as_str(),
            CollectionLayout::RootFiles.as_str()
        );
    }
}
