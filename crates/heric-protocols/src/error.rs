//! Errors for the non-HTTP protocols.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Something a protocol could not do.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// The request's URL cannot be parsed for this protocol.
    #[error("invalid {protocol} URL: {detail}")]
    InvalidUrl {
        /// Which protocol.
        protocol: &'static str,
        /// Why.
        detail: String,
    },

    /// The connection could not be established.
    #[error("cannot connect: {0}")]
    Connect(String),

    /// The handshake was answered with something other than an upgrade.
    ///
    /// The original surfaces this as an inspectable error response with the
    /// body dumped, which is why the status and body travel together.
    #[error("unexpected HTTP {status} during the {protocol} handshake")]
    Handshake {
        /// Which protocol.
        protocol: &'static str,
        /// The status.
        status: u16,
        /// The body, already decoded.
        body: String,
    },

    /// TLS failed for a protocol's own session.
    #[error("TLS failure: {0}")]
    Tls(String),

    /// A proto file could not be read or parsed.
    #[error("invalid proto: {0}")]
    Proto(String),

    /// Server reflection could not list the service.
    #[error("cannot reflect the service: {0}")]
    Reflection(String),

    /// The call returned a non-zero gRPC status.
    #[error("gRPC {code}: {message}")]
    Grpc {
        /// The status code.
        code: i32,
        /// The status message.
        message: String,
    },

    /// A message did not match the method's input type.
    #[error("cannot encode the request: {0}")]
    Encode(String),

    /// A response did not match the method's output type.
    #[error("cannot decode the response: {0}")]
    Decode(String),

    /// The session is no longer open.
    #[error("the session is closed")]
    Closed,

    /// The caller asked for something the protocol does not support.
    #[error("{0}")]
    Unsupported(String),

    /// A path was refused by the allowlist.
    #[error("path not allowed: {0}")]
    PathNotAllowed(String),

    /// The underlying transport failed.
    #[error("transport failure: {0}")]
    Transport(String),

    /// A session did not answer within its timeout.
    #[error("timed out after {0:?}")]
    Timeout(std::time::Duration),
}

impl Diagnostic for ProtocolError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::InvalidUrl { .. } => MessageKey::new("heric-protocols.invalid-url"),
            Self::Connect(_) => MessageKey::new("heric-protocols.connect"),
            Self::Handshake { .. } => MessageKey::new("heric-protocols.handshake"),
            Self::Tls(_) => MessageKey::new("heric-protocols.tls"),
            Self::Proto(_) => MessageKey::new("heric-protocols.proto"),
            Self::Reflection(_) => MessageKey::new("heric-protocols.reflection"),
            Self::Grpc { .. } => MessageKey::new("heric-protocols.grpc-status"),
            Self::Encode(_) => MessageKey::new("heric-protocols.encode"),
            Self::Decode(_) => MessageKey::new("heric-protocols.decode"),
            Self::Closed => MessageKey::new("heric-protocols.closed"),
            Self::Unsupported(_) => MessageKey::new("heric-protocols.unsupported"),
            Self::PathNotAllowed(_) => MessageKey::new("heric-protocols.path-not-allowed"),
            Self::Transport(_) => MessageKey::new("heric-protocols.transport"),
            Self::Timeout(_) => MessageKey::new("heric-protocols.timeout"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::InvalidUrl { protocol, detail } => vec![
                ("protocol".to_owned(), (*protocol).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
            Self::Handshake {
                protocol,
                status,
                body: _,
            } => vec![
                ("protocol".to_owned(), (*protocol).into()),
                ("status".to_owned(), i64::from(*status).into()),
            ],
            Self::Grpc { code, message } => vec![
                ("code".to_owned(), i64::from(*code).into()),
                ("message".to_owned(), message.as_str().into()),
            ],
            Self::Connect(detail)
            | Self::Tls(detail)
            | Self::Proto(detail)
            | Self::Reflection(detail)
            | Self::Encode(detail)
            | Self::Decode(detail)
            | Self::Unsupported(detail)
            | Self::PathNotAllowed(detail)
            | Self::Transport(detail) => vec![("detail".to_owned(), detail.as_str().into())],
            Self::Timeout(waited) => {
                vec![("seconds".to_owned(), (waited.as_secs() as i64).into())]
            }
            Self::Closed => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::Proto(_) | Self::Reflection(_) => {
                Some(MessageKey::new("heric-protocols.hint.check-proto"))
            }
            Self::Tls(_) => Some(MessageKey::new("heric-protocols.hint.check-certificate")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type ProtocolDiagnostic = ProtocolError;

impl From<ProtocolError> for heric_model::TimelineEntry {
    fn from(error: ProtocolError) -> Self {
        Self {
            at: std::time::Duration::ZERO,
            kind: heric_model::TimelineKind::Text,
            text: error.to_string(),
        }
    }
}
