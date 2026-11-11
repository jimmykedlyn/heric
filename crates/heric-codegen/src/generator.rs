//! Snippet generation contracts.

use heric_model::{Body, PreparedRequest};

use crate::{Client, Target};

/// How a value that came from a secret is treated.
///
/// The original masks confidential values with a placeholder that is random per
/// process, so a snippet the user copies cannot leak a token even if they paste
/// it somewhere public. That placeholder survives URL encoding, which is the
/// subtle part: encoding the mask would produce a different, unmatchable string
/// (`IO-017`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VariablePolicy {
    /// Write the rendered value. Only for a request with no secrets.
    Reveal,
    /// Substitute the mask for every value rendered from a secret.
    #[default]
    Mask,
    /// Write the template tag back, so the snippet carries no credential at all.
    TemplateTag,
}

/// A request prepared for snippet generation.
#[derive(Debug, Clone, Copy)]
pub struct SnippetRequest<'a> {
    /// The rendered request.
    pub request: &'a PreparedRequest,
    /// Which target.
    pub target: Target,
    /// Which client.
    pub client: Client,
    /// How to treat secrets.
    pub variables: VariablePolicy,
    /// Whether to include the body as a here-document for shell targets, which
    /// is what the reference implementation does for a payload over 2 KB.
    pub prefer_heredoc: bool,
}

impl<'a> SnippetRequest<'a> {
    /// A request with masking on and heredoc off, which is the modal's default.
    #[must_use]
    pub fn new(request: &'a PreparedRequest, target: Target, client: Client) -> Self {
        Self {
            request,
            target,
            client,
            variables: VariablePolicy::Mask,
            prefer_heredoc: false,
        }
    }

    /// Whether the request has a body to include.
    #[must_use]
    pub fn has_body(&self) -> bool {
        !matches!(self.request.body, Body::Empty)
    }
}

/// Produces snippets for one target and client.
pub trait SnippetGenerator: Send + Sync {
    /// Which target this handles.
    fn target(&self) -> Target;

    /// Which client this handles.
    fn client(&self) -> Client;

    /// Generates the snippet.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CodegenError`] when the request cannot be expressed in
    /// the target, such as a file upload for a client with no multipart support.
    fn generate(&self, request: &SnippetRequest<'_>) -> Result<String, crate::CodegenError>;
}

/// Every generator a build has.
#[derive(Default)]
pub struct Generators {
    generators: Vec<Box<dyn SnippetGenerator>>,
}

impl Generators {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a generator.
    pub fn register(&mut self, generator: Box<dyn SnippetGenerator>) {
        self.generators.push(generator);
    }

    /// Finds a generator for a pair.
    #[must_use]
    pub fn get(&self, target: Target, client: Client) -> Option<&dyn SnippetGenerator> {
        self.generators
            .iter()
            .find(|generator| generator.target() == target && generator.client() == client)
            .map(AsRef::as_ref)
    }

    /// How many generators are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.generators.len()
    }

    /// Whether none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.generators.is_empty()
    }
}

impl std::fmt::Debug for dyn SnippetGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnippetGenerator")
            .field("target", &self.target())
            .field("client", &self.client())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(body: Body) -> PreparedRequest {
        PreparedRequest {
            request_id: heric_model::Id::new("req_1"),
            kind: heric_model::DocKind::Request,
            method: "POST".to_owned(),
            url: "https://example.test/a".to_owned(),
            headers: Vec::new(),
            body,
        }
    }

    #[test]
    fn an_empty_body_reports_no_body() {
        let prepared = request(Body::Empty);
        let snippet = SnippetRequest::new(&prepared, Target::Curl, Client::Curl);
        assert!(!snippet.has_body());
    }

    #[test]
    fn a_text_body_reports_a_body() {
        let prepared = request(Body::text("{}", "application/json"));
        let snippet = SnippetRequest::new(&prepared, Target::Curl, Client::Curl);
        assert!(snippet.has_body());
    }

    #[test]
    fn masking_is_the_default() {
        let prepared = request(Body::Empty);
        assert_eq!(
            SnippetRequest::new(&prepared, Target::Curl, Client::Curl).variables,
            VariablePolicy::Mask
        );
    }
}
