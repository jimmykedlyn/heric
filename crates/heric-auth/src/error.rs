//! Errors raised while resolving credentials.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// A credential problem.
///
/// Every variant is translatable: the message keys live in `heric-auth.ftl` and
/// the GUI never sees English from this crate.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The request names a method with no provider.
    #[error("unsupported auth method: {0}")]
    Unsupported(String),

    /// The document's auth block does not have the expected shape.
    #[error("malformed auth block: {0}")]
    Shape(String),

    /// A required field is empty.
    #[error("{field} is required")]
    Missing {
        /// Which field.
        field: &'static str,
    },

    /// A secret was encrypted by the original and cannot be read.
    ///
    /// Not a failure the user can fix by retrying: the migration copied the
    /// ciphertext and the user re-enters it once (ADR 0011).
    #[error("{0} must be entered again")]
    PendingReEntry(String),

    /// The keychain refused to hand over a credential.
    #[error("keychain unavailable: {0}")]
    Keychain(String),

    /// No secret backend is available, and the request needs one.
    #[error("no secret storage available")]
    NoBackend,

    /// OAuth2 token acquisition failed.
    #[error("token request failed: {0}")]
    Token(String),

    /// The token store could not be read or written.
    #[error("token store error: {0}")]
    TokenStore(String),

    /// A signature could not be computed.
    #[error("cannot sign the request: {0}")]
    Signing(String),
}

impl Diagnostic for AuthError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Unsupported(_) => MessageKey::new("heric-auth.unsupported"),
            Self::Shape(_) => MessageKey::new("heric-auth.shape"),
            Self::Missing { .. } => MessageKey::new("heric-auth.missing-field"),
            Self::PendingReEntry(_) => MessageKey::new("heric-auth.pending-reentry"),
            Self::Keychain(_) => MessageKey::new("heric-auth.keychain"),
            Self::NoBackend => MessageKey::new("heric-auth.no-backend"),
            Self::Token(_) => MessageKey::new("heric-auth.token"),
            Self::TokenStore(_) => MessageKey::new("heric-auth.token-store"),
            Self::Signing(_) => MessageKey::new("heric-auth.signing"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Unsupported(detail) | Self::Shape(detail) | Self::Keychain(detail) => {
                vec![("detail".to_owned(), detail.as_str().into())]
            }
            Self::Missing { field } => vec![("field".to_owned(), (*field).into())],
            Self::PendingReEntry(what) => vec![("field".to_owned(), what.as_str().into())],
            Self::Token(detail) | Self::TokenStore(detail) | Self::Signing(detail) => {
                vec![("detail".to_owned(), detail.as_str().into())]
            }
            Self::NoBackend => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::PendingReEntry(_) => Some(MessageKey::new("heric-auth.hint.reenter")),
            Self::Keychain(_) | Self::NoBackend => {
                Some(MessageKey::new("heric-auth.hint.unlock-keychain"))
            }
            Self::Token(_) => Some(MessageKey::new("heric-auth.hint.check-oauth-settings")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type AuthDiagnostic = AuthError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_reentry_carries_a_hint() {
        let error = AuthError::PendingReEntry("token".to_owned());
        assert_eq!(error.message_key().as_str(), "heric-auth.pending-reentry");
        assert!(error.hint_key().is_some());
    }
}
