//! Authentication methods for Heric.
//!
//! Resolves the credentials a request carries and turns them into headers,
//! query parameters or engine-negotiated schemes. The split below matters: a
//! [`AuthProvider`] never reads a secret directly, it asks a
//! [`SecretResolver`], so a test can authenticate without a keychain and the
//! GUI can mark a credential as awaiting re-entry without the provider knowing
//! why (ADR 0011).

#![deny(unsafe_code)]

mod error;
mod method;
mod oauth2;
mod secret;

pub use error::{AuthDiagnostic, AuthError};
pub use method::{
    ApiKeyLocation, AuthContext, AuthMethod, AuthOutcome, AuthProvider, DigestChallenge, DigestQop,
    HawkAlgorithm, OAuth1SignatureMethod, ProviderRegistry,
};
pub use oauth2::{GrantType, OAuth2Token, TokenStore};
pub use secret::{
    MemorySecrets, SecretRef, SecretResolver, SecretResolverHandle, SecretState, UserPrompt,
};

use heric_model::Doc;

/// Returns the version of `heric-auth`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Builds a provider that can apply any supported method.
///
/// The registry is built rather than static because the AWS and OAuth2 providers
/// need settings the process supplies at startup.
///
/// # Errors
///
/// Returns [`AuthError::Unsupported`] if a method has no registered provider,
/// which for a complete build means a wiring mistake rather than user input.
pub fn providers() -> Result<ProviderRegistry, AuthError> {
    todo!("M1: register the per-method auth providers")
}

/// Whether a document's auth block names a method Heric can apply.
///
/// Used by the migration report: an import can carry a method from a newer
/// version of the original that Heric does not implement yet, and the user
/// should be told at import time rather than at send time.
///
/// # Errors
///
/// Returns [`AuthError::Shape`] when the auth block is not an object.
pub fn detect(doc: &Doc) -> Result<Option<AuthMethod>, AuthError> {
    let Some(auth) = doc.body.get("authentication") else {
        return Ok(None);
    };
    if auth.is_null() {
        return Ok(None);
    }
    AuthMethod::from_json(auth).map(Some)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
