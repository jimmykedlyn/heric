//! OAuth 2: grants, tokens and the token cache.

use heric_model::Id;

/// The OAuth 2 grants the original offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantType {
    /// The implicit flow, which returns a token directly from the authorise
    /// endpoint and never sees a refresh token.
    Implicit {
        /// The authorisation URL.
        authorization_url: String,
        /// The client id.
        client_id: String,
        /// Scopes requested at authorisation time.
        scope: Vec<String>,
        /// Where the provider redirects, or the textual `urn:ietf:wg:oauth:2.0:oob`
        /// marker for out-of-band.
        redirect_url: String,
        /// Extra authorisation parameters.
        extra: Vec<(String, String)>,
    },
    /// The authorisation-code flow, optionally with PKCE.
    AuthorizationCode {
        /// The authorisation URL.
        authorization_url: String,
        /// The token URL.
        access_token_url: String,
        /// The client id.
        client_id: String,
        /// The client secret, empty for a public client.
        client_secret: String,
        /// Where the provider redirects.
        redirect_url: String,
        /// Scopes.
        scope: Vec<String>,
        /// Whether PKCE is used.
        pkce: PkceMethod,
        /// Extra authorisation parameters.
        authorization_extra: Vec<(String, String)>,
        /// Extra token parameters.
        token_extra: Vec<(String, String)>,
    },
    /// Resource-owner password credentials.
    Password {
        /// The token URL.
        access_token_url: String,
        /// The client id.
        client_id: String,
        /// The client secret.
        client_secret: String,
        /// The username.
        username: String,
        /// The password.
        password: String,
        /// Scopes.
        scope: Vec<String>,
    },
    /// Client credentials.
    ClientCredentials {
        /// The token URL.
        access_token_url: String,
        /// The client id.
        client_id: String,
        /// The client secret.
        client_secret: String,
        /// Scopes.
        scope: Vec<String>,
    },
}

/// How PKCE is applied to an authorisation-code exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceMethod {
    /// No PKCE.
    None,
    /// `S256`, the default when PKCE is on.
    S256,
    /// Plain, which the original still allows.
    Plain,
}

impl GrantType {
    /// Reads the grant from an `oauth2` auth block.
    ///
    /// Never fails. An unrecognised grant becomes
    /// [`GrantType::ClientCredentials`], which is what the original does: the
    /// token request then fails with a server error the user can read, which is
    /// more useful than rejecting the document at import time.
    #[must_use]
    pub fn from_json(object: &serde_json::Map<String, serde_json::Value>) -> Self {
        let text = |key: &str| -> String {
            object
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let list = |key: &str| -> Vec<String> {
            object
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|value| {
                    value
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        match text("grant_type").as_str() {
            "implicit" => Self::Implicit {
                authorization_url: text("authorizationUrl"),
                client_id: text("clientId"),
                scope: list("scope"),
                redirect_url: text("redirectUrl"),
                extra: Vec::new(),
            },
            "authorization_code" => Self::AuthorizationCode {
                authorization_url: text("authorizationUrl"),
                access_token_url: text("accessTokenUrl"),
                client_id: text("clientId"),
                client_secret: text("clientSecret"),
                redirect_url: text("redirectUrl"),
                scope: list("scope"),
                pkce: match text("pkceMethod").as_str() {
                    "S256" => PkceMethod::S256,
                    "plain" => PkceMethod::Plain,
                    _ if text("usePkce").eq_ignore_ascii_case("true") => PkceMethod::S256,
                    _ => PkceMethod::None,
                },
                authorization_extra: Vec::new(),
                token_extra: Vec::new(),
            },
            "password" => Self::Password {
                access_token_url: text("accessTokenUrl"),
                client_id: text("clientId"),
                client_secret: text("clientSecret"),
                username: text("username"),
                password: text("password"),
                scope: list("scope"),
            },
            _ => Self::ClientCredentials {
                access_token_url: text("accessTokenUrl"),
                client_id: text("clientId"),
                client_secret: text("clientSecret"),
                scope: list("scope"),
            },
        }
    }

    /// Whether a refresh token is available for this grant.
    ///
    /// Implicit never issues one, which is why the original cannot refresh an
    /// implicit token and asks the user to authorise again.
    #[must_use]
    pub fn can_refresh(&self) -> bool {
        matches!(
            self,
            Self::AuthorizationCode { .. } | Self::Password { .. } | Self::ClientCredentials { .. }
        )
    }

    /// Whether the flow needs a browser round trip through the authorisation
    /// endpoint.
    #[must_use]
    pub fn needs_browser(&self) -> bool {
        matches!(self, Self::Implicit { .. } | Self::AuthorizationCode { .. })
    }
}

/// An access token and its refresh token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuth2Token {
    /// The access token.
    pub access_token: String,
    /// The refresh token, when the provider sent one.
    pub refresh_token: Option<String>,
    /// Token type, `Bearer` by default.
    pub token_type: Option<String>,
    /// Lifetime in seconds.
    pub expires_in: Option<i64>,
    /// Space-separated scopes the provider granted.
    pub scope: Option<String>,
    /// When it was obtained, Unix seconds. Used to decide whether a cached token
    /// is still good.
    pub obtained_at: i64,
    /// Whether the exchange succeeded, `false` for a failed token request whose
    /// details live in `error`/`error_description`.
    pub success: bool,
    /// The provider's error code.
    pub error: Option<String>,
    /// The provider's error description.
    pub error_description: Option<String>,
    /// Debug: the id of the token request's response.
    pub x_response_id: Option<String>,
    /// Debug: the error the token request produced.
    pub x_error: Option<String>,
}

impl OAuth2Token {
    /// Whether the token is expired, or close enough that a refresh is due.
    ///
    /// The original refreshes a token that is within 60 seconds of expiry, so a
    /// request does not fail on a token that expires mid-flight.
    #[must_use]
    pub fn is_expired(&self, now: i64) -> bool {
        match self.expires_in {
            Some(expires_in) if expires_in > 0 => now >= self.obtained_at + expires_in - 60,
            _ => false,
        }
    }
}

/// Where tokens are cached.
///
/// Per request and per folder, so a folder-level token is inherited by its
/// requests, matching the original's `OAuth2Token` documents (`MOD-057`).
pub trait TokenStore: Send + Sync {
    /// A cached token for a document, falling back to its ancestors.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError::TokenStore`] when the store cannot be read.
    fn get(&self, doc: &Id) -> Result<Option<OAuth2Token>, crate::AuthError>;

    /// Caches a token.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError::TokenStore`] when the store cannot be written.
    fn put(&self, doc: &Id, token: &OAuth2Token) -> Result<(), crate::AuthError>;

    /// Drops a token, used when the server answers 401 or `invalid_grant`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError::TokenStore`] when the store cannot be written.
    fn invalidate(&self, doc: &Id) -> Result<(), crate::AuthError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn pkce_defaults_to_s256_when_enabled() {
        let grant = GrantType::from_json(&block(serde_json::json!({
            "grant_type": "authorization_code",
            "usePkce": "true"
        })));
        assert!(matches!(
            grant,
            GrantType::AuthorizationCode {
                pkce: PkceMethod::S256,
                ..
            }
        ));
    }

    #[test]
    fn implicit_cannot_refresh() {
        let grant = GrantType::from_json(&block(serde_json::json!({ "grant_type": "implicit" })));
        assert!(!grant.can_refresh());
        assert!(grant.needs_browser());
    }

    #[test]
    fn an_unknown_grant_falls_back_to_client_credentials() {
        let grant = GrantType::from_json(&block(serde_json::json!({
            "grant_type": "device_code",
            "accessTokenUrl": "https://example.test/token"
        })));
        assert!(matches!(grant, GrantType::ClientCredentials { .. }));
    }

    #[test]
    fn a_token_expires_with_a_sixty_second_margin() {
        let token = OAuth2Token {
            access_token: "a".to_owned(),
            refresh_token: None,
            token_type: None,
            expires_in: Some(3600),
            scope: None,
            obtained_at: 1_000,
            success: true,
            error: None,
            error_description: None,
            x_response_id: None,
            x_error: None,
        };
        assert!(!token.is_expired(1_000));
        assert!(!token.is_expired(4_500));
        assert!(token.is_expired(4_600));
        assert!(token.is_expired(9_999));
    }
}
