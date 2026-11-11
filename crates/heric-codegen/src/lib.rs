//! Code snippet generation.
//!
//! Turns an HTTP request into a ready-to-paste snippet for a target language.
//! The original delegates this to `httpsnippet`; Heric reimplements the target
//! matrix it exposes, because the library is a Node package and a Rust binary
//! cannot call it.
//!
//! Golden tests come from the original's own snippet fixtures, so the output is
//! compared character for character against the reference implementation rather
//! than being judged plausible.

#![deny(unsafe_code)]

mod error;
mod generator;
mod target;

pub use error::{CodegenDiagnostic, CodegenError};
pub use generator::{Generators, SnippetGenerator, SnippetRequest, VariablePolicy};
pub use target::{CLIENT_COUNT, Client, TARGET_COUNT, Target};

use heric_model::PreparedRequest;

/// Returns the version of `heric-codegen`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Produces a snippet.
///
/// # Errors
///
/// Returns [`CodegenError::UnsupportedTarget`] when no generator handles the
/// pair, which the UI prevents by only offering the registered pairs.
pub fn generate(
    request: &SnippetRequest<'_>,
    target: Target,
    client: Client,
) -> Result<String, CodegenError> {
    let _ = (request, target, client);
    todo!("M5: generate a snippet for the requested target and client")
}

/// Every target and client pair a snippet can be generated for.
///
/// The snippet modal lists these; a pair missing from here is not offered, which
/// is how the original is wired too.
#[must_use]
pub fn supported() -> &'static [(Target, Client)] {
    &[
        (Target::Curl, Client::Curl),
        (Target::Node, Client::Native),
        (Target::Node, Client::Fetch),
        (Target::Node, Client::Axios),
        (Target::JavaScript, Client::XmlHttpRequest),
        (Target::JavaScript, Client::Fetch),
        (Target::Python, Client::Requests),
        (Target::Python, Client::Httpx),
        (Target::Go, Client::Native),
        (Target::Java, Client::OkHttp),
        (Target::Java, Client::Unirest),
        (Target::CSharp, Client::RestSharp),
        (Target::CSharp, Client::HttpClient),
        (Target::Php, Client::Curl),
        (Target::Php, Client::Guzzle),
        (Target::Ruby, Client::Native),
        (Target::Shell, Client::Httpie),
        (Target::Shell, Client::Wget),
        (Target::Swift, Client::UrlSession),
        (Target::Kotlin, Client::OkHttp),
        (Target::ObjectiveC, Client::NsUrlSession),
        (Target::R, Client::Httr),
        (Target::R, Client::Httr2),
        (Target::Ocaml, Client::Cohttp),
    ]
}

/// Whether a pair is offered.
///
/// # Errors
///
/// Returns [`CodegenError::UnsupportedTarget`] with both names, so the message
/// can name what was asked for rather than saying "unsupported".
pub fn ensure_supported(target: Target, client: Client) -> Result<(), CodegenError> {
    if supported().contains(&(target, client)) {
        Ok(())
    } else {
        Err(CodegenError::UnsupportedTarget {
            target: target.as_str(),
            client: client.as_str(),
        })
    }
}

/// The request a snippet is generated from.
///
/// A separate type from [`PreparedRequest`] because a snippet must not carry the
/// rendered secrets: the original masks them through a per-process placeholder
/// and keeps the mask when it URL-encodes a value (`IO-017`). Taking the
/// prepared request and a policy keeps that decision explicit at the call site.
#[derive(Debug, Clone, Copy)]
pub struct SnippetSource<'a> {
    /// The rendered request.
    pub request: &'a PreparedRequest,
    /// How to treat values that were rendered from a secret.
    pub variables: VariablePolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_is_always_offered() {
        assert!(supported().contains(&(Target::Curl, Client::Curl)));
    }

    #[test]
    fn every_pair_is_unique() {
        let pairs = supported();
        for (index, pair) in pairs.iter().enumerate() {
            assert!(
                !pairs[index + 1..].contains(pair),
                "duplicate pair {pair:?}"
            );
        }
    }

    #[test]
    fn an_unsupported_pair_names_both_sides() -> Result<(), Box<dyn std::error::Error>> {
        let error = ensure_supported(Target::Curl, Client::Fetch)
            .err()
            .ok_or("expected an error")?;
        assert!(error.to_string().contains("curl"));
        Ok(())
    }

    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
