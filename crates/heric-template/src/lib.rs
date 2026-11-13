//! Template rendering for Heric: Liquid semantics, plus the template tags the
//! original client defines.
//!
//! "Nunjucks" is the name the original codebase uses for this engine. The
//! language it actually implements is Liquid, configured to look like
//! Nunjucks: Liquid filters, `strictVariables` on by default, JavaScript
//! truthiness and a dual root/`_` context. ADR 0003 records why that
//! distinction matters.
//!
//! The renderer is synchronous and pure apart from the [`TagHost`] calls, so
//! the code that renders a preview and the code that renders before a send are
//! the same code.

#![deny(unsafe_code)]

mod ast;
mod context;
mod error;
mod eval;
pub mod filters;
mod host;
mod lexer;
mod parser;
mod purpose;
mod tag;
pub mod tags;
pub mod testing;
mod truthy;

pub use context::{ConfidentialPolicy, RenderContext, Scope};
pub use error::{RenderError, RenderErrorKind};
pub use host::{PromptAnswer, PromptRequest, ReadPurpose, TagHost, Timeout};
pub use purpose::RenderPurpose;
pub use tag::{TagDefinition, TagError, tags};

/// Renders a template string.
///
/// # Errors
///
/// Returns [`RenderError`] when the template does not parse or evaluation
/// fails. Undefined-variable failures are reported as
/// [`RenderErrorKind::UndefinedVariable`], which the UI branches on.
pub fn render(
    input: &str,
    context: &RenderContext<'_>,
    host: &dyn TagHost,
) -> Result<String, RenderError> {
    // The original short-circuits before parsing when there is nothing to
    // parse; doing the same keeps the common case free of lexing work.
    if !has_template_syntax(input) {
        return Ok(input.to_owned());
    }
    let nodes = parser::parse(input)?;
    eval::eval(&nodes, context, host)
}

/// Renders every templated field of a request in place.
///
/// Which fields are rendered, in which order, and which are left alone is the
/// contract this function exists to hold: the original renders URL, query
/// parameters, headers, body text, path parameters and the authentication
/// block, and never re-renders a cookie.
///
/// # Errors
///
/// Returns [`RenderError`] and leaves the document unmodified on failure, so a
/// half-rendered request never reaches the engine.
pub fn render_request(
    request: &mut heric_model::Doc,
    context: &RenderContext<'_>,
    host: &dyn TagHost,
) -> Result<(), RenderError> {
    let _ = (request, context, host);
    todo!("M1: render the request's templated fields atomically")
}

/// Reports whether a string contains template syntax.
///
/// The original short-circuits before parsing when it finds none, which is what
/// keeps the common case cheap.
#[must_use]
pub fn has_template_syntax(input: &str) -> bool {
    parser::has_tags(input)
}

/// Returns the version of `heric-template`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_both_tag_forms() {
        assert!(has_template_syntax("{{ base_url }}/users"));
        assert!(has_template_syntax("{% uuid 'v4' %}"));
        assert!(!has_template_syntax("https://example.com"));
    }
}
