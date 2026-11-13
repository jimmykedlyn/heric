//! Helpers for tests in this crate and in `heric-core`.
//!
//! `heric-core` needs a `TagHost` that refuses everything, so its tests can
//! render without a network or a filesystem. Keeping it here means the refusal
//! is written once and cannot drift between crates.

use std::path::Path;
use std::sync::OnceLock;

use heric_model::{Execution, PreparedRequest};
use serde_json::Value;

use crate::context::{ConfidentialPolicy, RenderContext, Scope};
use crate::host::{PromptAnswer, PromptRequest, ReadPurpose, TagHost};
use crate::purpose::RenderPurpose;
use crate::tag::TagError;

/// A host that refuses every request to leave the renderer.
///
/// Rendering a preview must not open a socket, read a file or wait for a
/// person. A test that needs one of those has to supply a real host.
pub struct NoHost;

impl TagHost for NoHost {
    fn read_file(&self, path: &Path, _purpose: ReadPurpose) -> Result<Vec<u8>, TagError> {
        Err(TagError::PathNotAllowed {
            path: path.display().to_string(),
        })
    }

    fn prompt(&self, request: &PromptRequest) -> Result<PromptAnswer, TagError> {
        let _ = request;
        Err(TagError::NotAvailableInThisPurpose { tag: "prompt" })
    }

    fn send(&self, request: &PreparedRequest) -> Result<Execution, TagError> {
        let _ = request;
        Err(TagError::PathNotAllowed {
            path: "<send>".to_owned(),
        })
    }

    fn purpose(&self) -> RenderPurpose {
        // `Export` is the most restrictive purpose that is not `NoRender`, so a
        // test that accidentally needs the host fails loudly instead of
        // blocking.
        RenderPurpose::Export
    }
}

/// A context whose collection environment is `scope` and whose other six
/// layers are empty.
///
/// `ConfidentialPolicy::Reveal` because a test that does not care about
/// masking should see the real values.
#[must_use]
pub fn context(scope: &Scope) -> RenderContext<'_> {
    static EMPTY: OnceLock<Scope> = OnceLock::new();
    RenderContext {
        global_root: EMPTY.get_or_init(Scope::new),
        global_sub: None,
        collection_root: scope,
        collection_sub: None,
        folder_ancestors: &[],
        iteration: None,
        transient: None,
        policy: ConfidentialPolicy::Reveal,
        purpose: RenderPurpose::Preview,
        request_id: None,
    }
}

/// A context holding `pairs` as the collection environment.
///
/// The scope has to outlive the context, and a `RenderContext` borrows it, so
/// the scope is stored in a `OnceLock` that lives for the process. One lock per
/// test process is enough because a test only reads the context it built; two
/// tests wanting different pairs in the same process would race, which is why
/// this is a testing convenience and not a public constructor.
///
/// # Panics
///
/// Panics only if called twice with different pairs in one process. Prefer
/// [`context`] with a local `Scope` when a test needs its own values.
#[must_use]
pub fn context_with(pairs: &[(&str, Value)]) -> RenderContext<'static> {
    static SHARED: OnceLock<Scope> = OnceLock::new();
    let mut variables = serde_json::Map::new();
    for (name, value) in pairs {
        variables.insert((*name).to_owned(), value.clone());
    }
    context(SHARED.get_or_init(|| Scope::from_map(variables)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_context_holds_its_scope() {
        let scope = Scope::from_map(serde_json::Map::from_iter([(
            "name".to_owned(),
            json!("kyle"),
        )]));
        let context = context(&scope);
        assert_eq!(context.merged().get("name"), Some(&json!("kyle")));
    }

    #[test]
    fn no_host_refuses_a_prompt() {
        let request = PromptRequest {
            request_id: None,
            title: "q".to_owned(),
            description: None,
            default_value: None,
            secret: false,
            timeout: std::time::Duration::from_secs(0),
        };
        assert!(matches!(
            NoHost.prompt(&request),
            Err(TagError::NotAvailableInThisPurpose { .. })
        ));
    }
}
