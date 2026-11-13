//! The render context: layered scopes and the confidentiality policy.

use heric_model::Id;

use crate::RenderPurpose;

/// A named layer of variables.
///
/// The original layers seven sources, in this order of increasing precedence:
/// global root, global sub, collection root, collection sub, folder ancestors,
/// iteration variables, transient values. Each layer is a JSON object of
/// name-to-value pairs, and tags read either the root or the `_` object.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// An environment's variables.
    pub variables: serde_json::Map<String, serde_json::Value>,
    /// The environment these came from, when known.
    pub environment: Option<Id>,
}

impl Scope {
    /// An empty scope.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A scope from a map of values.
    #[must_use]
    pub fn from_map(variables: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            variables,
            environment: None,
        }
    }

    /// Whether the scope holds no variables.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }
}

/// Everything a render reads.
#[derive(Debug, Clone)]
pub struct RenderContext<'a> {
    /// Global (project-wide) root variables.
    pub global_root: &'a Scope,
    /// Global sub-environment variables.
    pub global_sub: Option<&'a Scope>,
    /// Collection base-environment variables.
    pub collection_root: &'a Scope,
    /// Collection sub-environment variables.
    pub collection_sub: Option<&'a Scope>,
    /// Folder ancestors, nearest last.
    pub folder_ancestors: &'a [Scope],
    /// Iteration variables, as the runner sets them.
    pub iteration: Option<&'a Scope>,
    /// Transient values, such as a `{% prompt %}` answer reused within one
    /// render.
    pub transient: Option<&'a Scope>,
    /// What may be revealed during this render.
    pub policy: ConfidentialPolicy,
    /// Why this render is happening.
    pub purpose: RenderPurpose,
    /// The request being rendered, when there is one, for `{% request %}`.
    pub request_id: Option<Id>,
}

/// What a render is allowed to do with values marked confidential.
///
/// The original masks a confidential value before a fetch and reveals it only
/// for the purpose that needs the real value. Getting this wrong leaks secrets
/// into a preview window, so it is a required part of the context rather than a
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidentialPolicy {
    /// Reveal real values. Only for a send or a script that must reach the
    /// network.
    Reveal,
    /// Replace every confidential value with the mask the original uses.
    Mask,
    /// Fail the render when a template references a confidential value.
    Deny,
}

impl RenderContext<'_> {
    /// Every layer, merged into one map, lowest precedence first.
    ///
    /// The original spreads the seven layers in this order and lets later
    /// ones win. Order matters: a folder ancestor overrides the collection
    /// environment, and an iteration variable overrides both.
    #[must_use]
    pub fn merged(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut merged = serde_json::Map::new();
        for layer in [
            Some(self.global_root),
            self.global_sub,
            Some(self.collection_root),
            self.collection_sub,
        ]
        .into_iter()
        .flatten()
        {
            merged.extend(layer.variables.clone());
        }
        for ancestor in self.folder_ancestors {
            merged.extend(ancestor.variables.clone());
        }
        for layer in [self.iteration, self.transient].into_iter().flatten() {
            merged.extend(layer.variables.clone());
        }
        merged
    }
}

impl ConfidentialPolicy {
    /// The policy a purpose implies.
    #[must_use]
    pub fn for_purpose(purpose: RenderPurpose) -> Self {
        match purpose {
            RenderPurpose::Send | RenderPurpose::Script => Self::Reveal,
            RenderPurpose::Preview | RenderPurpose::Export => Self::Mask,
            RenderPurpose::NoRender => Self::Deny,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_scope_is_empty() {
        assert!(Scope::new().is_empty());
    }

    #[test]
    fn preview_masks_and_send_reveals() {
        assert_eq!(
            ConfidentialPolicy::for_purpose(RenderPurpose::Preview),
            ConfidentialPolicy::Mask
        );
        assert_eq!(
            ConfidentialPolicy::for_purpose(RenderPurpose::Send),
            ConfidentialPolicy::Reveal
        );
    }
}
