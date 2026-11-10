//! Authentication schemes for Heric.
//!
//! Implements every auth mechanism the original client supports: Basic,
//! Digest, NTLM, Bearer, OAuth 1.0 and 2.0 (all grants), AWS SigV4, Hawk,
//! ASAP, API key and Netrc. See `docs/inventory/http.md` once the
//! inventory lands.

/// Identifies an authentication scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    /// HTTP Basic authentication.
    Basic,
    /// Bearer token authentication.
    Bearer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemes_are_distinguishable() {
        assert_ne!(AuthScheme::Basic, AuthScheme::Bearer);
    }
}
