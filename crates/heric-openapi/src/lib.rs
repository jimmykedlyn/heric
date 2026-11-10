//! OpenAPI support for Heric.
//!
//! Parses and lints OpenAPI specifications and backs Design mode. See
//! `docs/inventory/io-and-cli.md` once the inventory lands.

/// Error type for OpenAPI parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The document is not valid JSON or YAML.
    #[error("document is not valid JSON or YAML")]
    InvalidSyntax,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_syntax_message_is_stable() {
        assert_eq!(
            ParseError::InvalidSyntax.to_string(),
            "document is not valid JSON or YAML"
        );
    }
}
