//! Authentication methods and how they are applied to a request.

use heric_model::Doc;

use crate::{AuthError, SecretRef, SecretResolver};

/// Where an API key goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyLocation {
    /// An `Authorization`-style header.
    Header,
    /// A query parameter.
    Query,
    /// A cookie.
    Cookie,
}

/// Query-protection options for HTTP Digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestQop {
    /// No `qop` at all.
    None,
    /// `auth` only.
    Auth,
    /// `auth-int`, which hashes the body too.
    AuthInt,
}

/// A Digest challenge parsed from a `WWW-Authenticate` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestChallenge {
    /// The realm.
    pub realm: String,
    /// The server nonce.
    pub nonce: String,
    /// The opaque value, echoed back unchanged.
    pub opaque: Option<String>,
    /// Accepted qop values, empty when the server sent none.
    pub qop: Vec<DigestQop>,
    /// The hashing algorithm.
    pub algorithm: String,
    /// Whether the nonce was issued for a stale session.
    pub stale: bool,
}

/// Hawk's hashing algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HawkAlgorithm {
    /// SHA-1.
    Sha1,
    /// SHA-256.
    Sha256,
}

/// How an OAuth 1.0a signature is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuth1SignatureMethod {
    /// HMAC-SHA1.
    HmacSha1,
    /// HMAC-SHA256.
    HmacSha256,
    /// RSA-SHA1.
    RsaSha1,
    /// No signature.
    Plaintext,
}

/// The authentication methods the original supports.
///
/// Kept as data rather than as one type per method because the set is fixed by
/// the original's schema and a new method arriving in a document must degrade to
/// [`AuthMethod::Unknown`] instead of failing to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// No authentication.
    None,
    /// Bearer token.
    Bearer {
        /// Token, resolved through a [`SecretResolver`] when it is a reference.
        token: String,
        /// Prefix prepended to the token, `Bearer` by default.
        prefix: String,
    },
    /// HTTP Basic.
    Basic {
        /// Username.
        username: String,
        /// Password.
        password: String,
    },
    /// API key in a header, query parameter or cookie.
    ApiKey {
        /// Where it goes.
        location: ApiKeyLocation,
        /// Header or parameter name.
        name: String,
        /// The key.
        key: String,
    },
    /// Digest, negotiated from a challenge.
    Digest {
        /// Username.
        username: String,
        /// Password.
        password: String,
    },
    /// NTLM.
    ///
    /// Depends on libcurl's NTLM support, which outside Windows is a P0 risk
    /// carried in ADR 0004.
    Ntlm {
        /// Username.
        username: String,
        /// Password.
        password: String,
        /// Domain.
        domain: Option<String>,
    },
    /// Credentials read from a `.netrc` file.
    Netrc {
        /// Whether to use the machine's own `.netrc`.
        use_machine_netrc: bool,
        /// Contents of a workspace netrc, when one is supplied.
        netrc: Option<String>,
    },
    /// AWS Signature v4.
    AwsIam {
        /// Access key id.
        access_key_id: String,
        /// Secret access key.
        secret_access_key: String,
        /// Region.
        region: String,
        /// Service name, `execute-api` by default.
        service: String,
        /// Session token, when temporary credentials are used.
        session_token: Option<String>,
    },
    /// OAuth 1.0a.
    OAuth1 {
        /// Consumer key.
        consumer_key: String,
        /// Consumer secret.
        consumer_secret: String,
        /// Token.
        token: String,
        /// Token secret.
        token_secret: String,
        /// Signature method.
        signature_method: OAuth1SignatureMethod,
        /// Realm.
        realm: Option<String>,
        /// Additional signature parameters.
        extra: Vec<(String, String)>,
    },
    /// OAuth 2.
    OAuth2 {
        /// Which grant.
        grant: crate::GrantType,
        /// Whether the token request validates TLS.
        validate_auth_ssl: bool,
    },
    /// Hawk.
    Hawk {
        /// Auth id.
        auth_id: String,
        /// Auth key.
        auth_key: String,
        /// Algorithm.
        algorithm: HawkAlgorithm,
        /// Whether the body is hashed into the MAC.
        validate_payload: bool,
        /// Extra `ext` value.
        ext: Option<String>,
        /// Application id.
        app: Option<String>,
        /// Delegation, `None` when absent.
        delegation: Option<String>,
    },
    /// ASAP (a JWT asserted to the recipient).
    Asap {
        /// Issuer.
        issuer: String,
        /// Audience.
        audience: String,
        /// Key id.
        key_id: String,
        /// Private key, PEM.
        private_key: String,
        /// Token lifetime in seconds.
        expiry: Option<i64>,
        /// Additional claims.
        claims: Vec<(String, String)>,
    },
    /// A method this build does not know.
    Unknown {
        /// The `type` string found in the document.
        kind: String,
    },
}

impl AuthMethod {
    /// Parses the `authentication` object of a document.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Shape`] when the value is not an object. An unknown
    /// `type` is not an error: it becomes [`AuthMethod::Unknown`] so an import
    /// keeps the document instead of dropping it.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, AuthError> {
        let object = value
            .as_object()
            .ok_or_else(|| AuthError::Shape("authentication is not an object".to_owned()))?;
        let kind = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(AuthError::Missing { field: "type" })?;
        let text = |key: &str| -> String {
            object
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(match kind {
            "none" => Self::None,
            "bearer" => Self::Bearer {
                token: text("token"),
                prefix: text("prefix"),
            },
            "basic" => Self::Basic {
                username: text("username"),
                password: text("password"),
            },
            "apikey" => Self::ApiKey {
                location: match text("in").as_str() {
                    "query" => ApiKeyLocation::Query,
                    "cookie" => ApiKeyLocation::Cookie,
                    _ => ApiKeyLocation::Header,
                },
                name: text("key"),
                key: text("value"),
            },
            "digest" => Self::Digest {
                username: text("username"),
                password: text("password"),
            },
            "ntlm" => Self::Ntlm {
                username: text("username"),
                password: text("password"),
                domain: object
                    .get("domain")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            },
            "netrc" => Self::Netrc {
                use_machine_netrc: object
                    .get("useMachineNetrc")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                netrc: object
                    .get("netrc")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            },
            "iam" => Self::AwsIam {
                access_key_id: text("accessKeyId"),
                secret_access_key: text("secretAccessKey"),
                region: text("region"),
                service: text("service"),
                session_token: object
                    .get("sessionToken")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            },
            "oauth1" => Self::OAuth1 {
                consumer_key: text("consumerKey"),
                consumer_secret: text("consumerSecret"),
                token: text("tokenKey"),
                token_secret: text("tokenSecret"),
                signature_method: match text("signatureMethod").as_str() {
                    "HMAC-SHA256" => OAuth1SignatureMethod::HmacSha256,
                    "RSA-SHA1" => OAuth1SignatureMethod::RsaSha1,
                    "PLAINTEXT" => OAuth1SignatureMethod::Plaintext,
                    _ => OAuth1SignatureMethod::HmacSha1,
                },
                realm: object
                    .get("realm")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                extra: Vec::new(),
            },
            "oauth2" => Self::OAuth2 {
                grant: crate::GrantType::from_json(object),
                validate_auth_ssl: object
                    .get("validateAuthSSL")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
            },
            "hawk" => Self::Hawk {
                auth_id: text("authId"),
                auth_key: text("authKey"),
                algorithm: match text("algorithm").as_str() {
                    "sha256" => HawkAlgorithm::Sha256,
                    _ => HawkAlgorithm::Sha1,
                },
                validate_payload: object
                    .get("validatePayload")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                ext: object
                    .get("ext")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                app: object
                    .get("app")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                delegation: object
                    .get("delegation")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            },
            "asap" => Self::Asap {
                issuer: text("issuer"),
                audience: text("audience"),
                key_id: text("keyId"),
                private_key: text("privateKey"),
                expiry: object.get("expiry").and_then(serde_json::Value::as_i64),
                claims: Vec::new(),
            },
            other => Self::Unknown {
                kind: other.to_owned(),
            },
        })
    }

    /// The `type` string the original would write for this method.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::None => "none",
            Self::Bearer { .. } => "bearer",
            Self::Basic { .. } => "basic",
            Self::ApiKey { .. } => "apikey",
            Self::Digest { .. } => "digest",
            Self::Ntlm { .. } => "ntlm",
            Self::Netrc { .. } => "netrc",
            Self::AwsIam { .. } => "iam",
            Self::OAuth1 { .. } => "oauth1",
            Self::OAuth2 { .. } => "oauth2",
            Self::Hawk { .. } => "hawk",
            Self::Asap { .. } => "asap",
            Self::Unknown { kind } => kind,
        }
    }

    /// Whether applying this method needs a secret from the keychain.
    #[must_use]
    pub fn needs_secret(&self) -> bool {
        matches!(
            self,
            Self::Bearer { .. }
                | Self::Basic { .. }
                | Self::ApiKey { .. }
                | Self::Digest { .. }
                | Self::Ntlm { .. }
                | Self::AwsIam { .. }
                | Self::OAuth1 { .. }
                | Self::OAuth2 { .. }
                | Self::Hawk { .. }
                | Self::Asap { .. }
        )
    }
}

/// What a provider is given.
pub struct AuthContext<'a> {
    /// The request being authenticated, after template rendering.
    pub request: &'a heric_model::PreparedRequest,
    /// The document it came from, for OAuth2 cache keys and folder inheritance.
    pub doc: &'a Doc,
    /// Secret lookup.
    pub secrets: &'a dyn SecretResolver,
    /// Where this document's secrets live.
    pub secret_ref: &'a SecretRef,
}

/// What a provider produced.
///
/// The original expresses every method as either headers or a query mutation,
/// except Digest and NTLM, which libcurl negotiates itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthOutcome {
    /// Headers to add or replace.
    pub headers: Vec<(String, String)>,
    /// Query parameters to add.
    pub query: Vec<(String, String)>,
    /// Cookies to add.
    pub cookies: Vec<(String, String)>,
    /// Whether the engine should let libcurl negotiate this scheme.
    pub engine_negotiated: Option<EngineScheme>,
}

/// Schemes libcurl negotiates after a challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineScheme {
    /// HTTP Digest.
    Digest,
    /// NTLM.
    Ntlm,
}

/// Applies one method.
///
/// Async-free on purpose: a provider may need a network round trip (OAuth2), and
/// that is handled by the caller asking for it explicitly before the blocking
/// send, so the engine's contract stays synchronous.
pub trait AuthProvider: Send + Sync {
    /// The method this provider handles, as a `type` string.
    fn type_name(&self) -> &'static str;

    /// Resolves credentials and computes the application.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError`] when a required credential is missing, pending
    /// re-entry or rejected by the backend.
    fn apply(
        &self,
        method: &AuthMethod,
        context: &AuthContext<'_>,
    ) -> Result<AuthOutcome, AuthError>;

    /// Methods whose credentials must be fetched before the send, in a runtime
    /// task rather than on the blocking pool.
    fn prefetch(&self, _method: &AuthMethod, _context: &AuthContext<'_>) -> Result<(), AuthError> {
        Ok(())
    }
}

/// The providers a build has.
#[derive(Debug, Default)]
pub struct ProviderRegistry {
    providers: Vec<Box<dyn AuthProvider>>,
}

impl ProviderRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a provider.
    pub fn register(&mut self, provider: Box<dyn AuthProvider>) {
        self.providers.push(provider);
    }

    /// Finds the provider for a method.
    #[must_use]
    pub fn get(&self, type_name: &str) -> Option<&dyn AuthProvider> {
        self.providers
            .iter()
            .find(|provider| provider.type_name() == type_name)
            .map(AsRef::as_ref)
    }

    /// Applies whatever method a document names.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Unsupported`] when no provider matches, and passes
    /// through the provider's own error otherwise.
    pub fn apply(
        &self,
        method: &AuthMethod,
        context: &AuthContext<'_>,
    ) -> Result<AuthOutcome, AuthError> {
        if matches!(method, AuthMethod::None) {
            return Ok(AuthOutcome::default());
        }
        let provider = self
            .get(method.type_name())
            .ok_or_else(|| AuthError::Unsupported(method.type_name().to_owned()))?;
        provider.apply(method, context)
    }

    /// How many providers are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether none are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl std::fmt::Debug for dyn AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthProvider")
            .field("type", &self.type_name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_type_is_kept_not_rejected() -> Result<(), AuthError> {
        let value = serde_json::json!({ "type": "saml" });
        let method = AuthMethod::from_json(&value)?;
        assert_eq!(
            method,
            AuthMethod::Unknown {
                kind: "saml".to_owned()
            }
        );
        assert_eq!(method.type_name(), "saml");
        Ok(())
    }

    #[test]
    fn a_bearer_block_parses() -> Result<(), AuthError> {
        let value = serde_json::json!({ "type": "bearer", "token": "abc", "prefix": "Token" });
        let method = AuthMethod::from_json(&value)?;
        assert!(
            matches!(method, AuthMethod::Bearer { ref token, ref prefix } if token == "abc" && prefix == "Token")
        );
        assert!(method.needs_secret());
        Ok(())
    }

    #[test]
    fn an_api_key_defaults_to_a_header() -> Result<(), AuthError> {
        let value = serde_json::json!({ "type": "apikey", "key": "X-Key", "value": "v" });
        let method = AuthMethod::from_json(&value)?;
        assert!(matches!(
            method,
            AuthMethod::ApiKey {
                location: ApiKeyLocation::Header,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn a_missing_type_is_an_error() {
        let value = serde_json::json!({});
        assert!(matches!(
            AuthMethod::from_json(&value),
            Err(AuthError::Missing { field: "type" })
        ));
    }

    #[test]
    fn none_needs_no_provider() -> Result<(), AuthError> {
        let registry = ProviderRegistry::new();
        let doc = Doc {
            meta: heric_model::DocMeta {
                id: heric_model::Id::new("req_1"),
                kind: heric_model::DocKind::Request,
                parent: None,
                modified: 0,
                created: 0,
                private: None,
                name: None,
            },
            body: serde_json::Map::new(),
        };
        let request = heric_model::PreparedRequest {
            request_id: doc.meta.id.clone(),
            kind: heric_model::DocKind::Request,
            method: "GET".to_owned(),
            url: "https://example.test".to_owned(),
            headers: Vec::new(),
            body: heric_model::Body::Empty,
        };
        let secrets = crate::MemorySecrets::new();
        let reference = SecretRef {
            doc: doc.meta.id.clone(),
            field: "token".to_owned(),
            service: "heric".to_owned(),
        };
        let context = AuthContext {
            request: &request,
            doc: &doc,
            secrets: &secrets,
            secret_ref: &reference,
        };
        assert_eq!(
            registry.apply(&AuthMethod::None, &context)?,
            AuthOutcome::default()
        );
        assert!(matches!(
            registry.apply(
                &AuthMethod::Unknown {
                    kind: "saml".to_owned()
                },
                &context
            ),
            Err(AuthError::Unsupported(_))
        ));
        Ok(())
    }
}
