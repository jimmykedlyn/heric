//! Turning a specification into a collection.

use heric_model::{Doc, DocKind};

use crate::{Operation, SecurityScheme, Spec};

/// How much of the specification to import.
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// Put each generated request in a folder named after its first tag.
    ///
    /// On by default: a specification with a hundred operations is unusable as
    /// a flat list, and the original groups by tag.
    pub group_by_tag: bool,
    /// Base URL to use instead of the specification's first server.
    pub base_url: Option<String>,
    /// Whether to seed request bodies from examples.
    pub use_examples: bool,
    /// Whether to import deprecated operations.
    pub include_deprecated: bool,
    /// Prefix for the generated folder, defaulting to the specification title.
    pub folder_name: Option<String>,
}

impl ConvertOptions {
    /// The defaults the import dialog starts from.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            group_by_tag: true,
            base_url: None,
            use_examples: true,
            include_deprecated: false,
            folder_name: None,
        }
    }
}

/// One generated request, before it becomes a document.
///
/// Kept separate from [`Doc`] so the conversion can be tested without a store,
/// and so the import report can describe what will be created before anything is
/// created.
#[derive(Debug, Clone)]
pub struct ConvertedRequest {
    /// The operation it came from.
    pub operation: Operation,
    /// The request's name, from `operationId` or the method and path.
    pub name: String,
    /// The tags that decided its folder.
    pub tags: Vec<String>,
}

/// A security scheme mapped onto a Heric environment variable.
///
/// The original turns security schemes into `{{ _.name }}` environment entries
/// rather than into request auth blocks, because a specification's security
/// applies to the collection rather than to one request. Keeping the mapping as
/// data lets the import report list what the user must fill in.
#[derive(Debug, Clone, PartialEq)]
pub struct SecurityMapping {
    /// The scheme's name in the specification.
    pub scheme: String,
    /// The environment variable name it becomes.
    pub variable: String,
    /// The scheme itself.
    pub kind: SecurityScheme,
    /// Whether the variable holds a secret, so its value is left blank and
    /// flagged for entry rather than written into the document.
    pub secret: bool,
}

/// A collection built from a specification but not yet persisted.
#[derive(Debug, Clone, Default)]
pub struct CollectionBuilder {
    /// The workspace that will own everything.
    pub workspace: Option<Doc>,
    /// The request groups, in creation order.
    pub groups: Vec<Doc>,
    /// The requests.
    pub requests: Vec<Doc>,
    /// The environment variables the user must fill in.
    pub variables: Vec<SecurityMapping>,
}

impl CollectionBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many documents the import will create.
    #[must_use]
    pub fn document_count(&self) -> usize {
        usize::from(self.workspace.is_some()) + self.groups.len() + self.requests.len()
    }

    /// Whether the import produced nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Every document, in an order where a parent always precedes its children.
    #[must_use]
    pub fn documents(&self) -> Vec<&Doc> {
        let mut all: Vec<&Doc> = Vec::with_capacity(self.document_count());
        all.extend(self.workspace.iter());
        all.extend(self.groups.iter());
        all.extend(self.requests.iter());
        all
    }

    /// The document kinds the import will create.
    #[must_use]
    pub fn kinds(&self) -> Vec<DocKind> {
        self.documents().iter().map(|doc| doc.kind()).collect()
    }
}

/// Builds a collection from a parsed specification.
///
/// # Errors
///
/// Returns [`crate::OpenApiError::Empty`] when the specification has no
/// operations.
pub fn build(
    spec: &Spec,
    options: &ConvertOptions,
) -> Result<CollectionBuilder, crate::OpenApiError> {
    let _ = (spec, options);
    todo!("M5: build a collection from a parsed specification")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_group_by_tag() {
        let options = ConvertOptions::defaults();
        assert!(options.group_by_tag);
        assert!(options.use_examples);
        assert!(!options.include_deprecated);
    }

    #[test]
    fn an_empty_builder_counts_nothing() {
        let builder = CollectionBuilder::new();
        assert_eq!(builder.document_count(), 0);
        assert!(builder.is_empty());
        assert!(builder.documents().is_empty());
    }
}
