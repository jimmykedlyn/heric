//! Errors for the HTTP engine.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Anything that prevents a response from existing.
///
/// A 4xx or 5xx status is not an error: it is a successful execution whose
/// status happens to be an error code. Transport failures that still produced
/// output are carried in `Execution::error` instead, because the original keeps
/// a failed response in history with its partial timeline.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// The caller cancelled the request.
    #[error("request cancelled")]
    Cancelled,

    /// A phase exceeded its timeout.
    #[error("{phase} timed out after {waited:?}")]
    Timeout {
        /// Which phase.
        phase: &'static str,
        /// How long it waited.
        waited: std::time::Duration,
    },

    /// The URL could not be parsed.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    /// The host could not be resolved.
    #[error("cannot resolve {0}")]
    Dns(String),

    /// The connection failed.
    #[error("cannot connect to {0}")]
    Connect(String),

    /// The TLS handshake failed.
    #[error("TLS failure: {0}")]
    Tls(String),

    /// The exchange broke after the connection was established.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// The redirect limit was reached.
    #[error("more than {0} redirects")]
    TooManyRedirects(u32),

    /// libcurl could not be initialised or used.
    #[error("engine error: {0}")]
    Engine(String),

    /// The stream's producer went away.
    #[error("stream closed")]
    StreamClosed,

    /// The engine does not implement the requested path.
    #[error("operation not supported by this engine")]
    Unsupported,

    /// A cookie jar could not be read or written.
    #[error("cookie jar error: {0}")]
    CookieJar(String),

    /// A certificate or key could not be loaded.
    #[error("cannot load {kind}: {detail}")]
    Certificate {
        /// What failed to load.
        kind: &'static str,
        /// Why.
        detail: String,
    },
}

impl Diagnostic for HttpError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Cancelled => MessageKey::new("heric-http.cancelled"),
            Self::Timeout { .. } => MessageKey::new("heric-http.timeout"),
            Self::InvalidUrl(_) => MessageKey::new("heric-http.invalid-url"),
            Self::Dns(_) => MessageKey::new("heric-http.dns"),
            Self::Connect(_) => MessageKey::new("heric-http.connect"),
            Self::Tls(_) => MessageKey::new("heric-http.tls"),
            Self::Protocol(_) => MessageKey::new("heric-http.protocol"),
            Self::TooManyRedirects(_) => MessageKey::new("heric-http.too-many-redirects"),
            Self::Engine(_) => MessageKey::new("heric-http.engine"),
            Self::StreamClosed => MessageKey::new("heric-http.stream-closed"),
            Self::Unsupported => MessageKey::new("heric-http.unsupported"),
            Self::CookieJar(_) => MessageKey::new("heric-http.cookie-jar"),
            Self::Certificate { .. } => MessageKey::new("heric-http.certificate"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Timeout { phase, waited } => vec![
                ("phase".to_owned(), (*phase).into()),
                ("seconds".to_owned(), (waited.as_secs() as i64).into()),
            ],
            Self::InvalidUrl(text)
            | Self::Dns(text)
            | Self::Connect(text)
            | Self::Tls(text)
            | Self::Protocol(text)
            | Self::Engine(text)
            | Self::CookieJar(text) => vec![("detail".to_owned(), text.as_str().into())],
            Self::TooManyRedirects(count) => {
                vec![("count".to_owned(), i64::from(*count).into())]
            }
            Self::Certificate { kind, detail } => vec![
                ("kind".to_owned(), (*kind).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Cancelled | Self::StreamClosed | Self::Unsupported => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::Dns(_) => Some(MessageKey::new("heric-http.hint.check-host")),
            Self::Tls(_) => Some(MessageKey::new("heric-http.hint.check-certificate")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type HttpDiagnostic = HttpError;
