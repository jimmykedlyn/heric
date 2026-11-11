//! Socket.IO.

use serde_json::Value;

/// A Socket.IO connection's settings.
///
/// The original supports a single auth shape, a single token, and keeps the
/// handshake path configurable with `/socket.io` as the default. Reproducing
/// that narrow surface is deliberate: the wider Socket.IO auth vocabulary exists
/// in the protocol but not in the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketIoConfig {
    /// The handshake path, `/socket.io` by default.
    pub path: String,
    /// Query parameters sent at handshake time.
    pub query: Vec<(String, String)>,
    /// Extra headers sent at handshake time.
    pub headers: Vec<(String, String)>,
    /// The token, authenticated as `{ "token": ... }`.
    pub token: Option<String>,
}

impl Default for SocketIoConfig {
    fn default() -> Self {
        Self {
            path: "/socket.io".to_owned(),
            query: Vec::new(),
            headers: Vec::new(),
            token: None,
        }
    }
}

impl SocketIoConfig {
    /// The `auth` payload sent with the connect packet.
    #[must_use]
    pub fn auth(&self) -> Option<Value> {
        self.token
            .as_ref()
            .map(|token| serde_json::json!({ "token": token }))
    }
}

/// One emit's arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketIoPayload {
    /// The event name, empty for a default emit.
    pub event: Option<String>,
    /// The arguments.
    ///
    /// Parsed from the editor's JSON. The original falls back to sending the raw
    /// string as a single argument when the JSON does not parse, rather than
    /// refusing to send (`PRO-056`), and Heric keeps that.
    pub args: Vec<SocketIoArgument>,
}

/// A single emitted argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketIoArgument {
    /// Parsed JSON.
    Json(Value),
    /// A raw string, used when the editor's content did not parse.
    Raw(String),
}

impl SocketIoPayload {
    /// Builds a payload from the editor's text, falling back to a raw argument.
    #[must_use]
    pub fn parse(event: Option<String>, text: &str) -> Self {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self {
                event,
                args: Vec::new(),
            };
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(Value::Array(items)) => Self {
                event,
                args: items.into_iter().map(SocketIoArgument::Json).collect(),
            },
            Ok(value) => Self {
                event,
                args: vec![SocketIoArgument::Json(value)],
            },
            Err(_) => Self {
                event,
                args: vec![SocketIoArgument::Raw(trimmed.to_owned())],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handshake_path_defaults_to_socket_io() {
        assert_eq!(SocketIoConfig::default().path, "/socket.io");
    }

    #[test]
    fn no_token_means_no_auth_payload() {
        assert_eq!(SocketIoConfig::default().auth(), None);
    }

    #[test]
    fn a_token_becomes_a_token_object() {
        let config = SocketIoConfig {
            token: Some("abc".to_owned()),
            ..SocketIoConfig::default()
        };
        assert_eq!(config.auth(), Some(serde_json::json!({ "token": "abc" })));
    }

    #[test]
    fn an_array_becomes_several_arguments() {
        let payload = SocketIoPayload::parse(Some("chat".to_owned()), r#"[1, "two"]"#);
        assert_eq!(payload.args.len(), 2);
        assert!(matches!(payload.args[0], SocketIoArgument::Json(_)));
    }

    #[test]
    fn invalid_json_is_sent_raw_instead_of_refused() {
        let payload = SocketIoPayload::parse(None, "hello {");
        assert_eq!(
            payload.args,
            vec![SocketIoArgument::Raw("hello {".to_owned())]
        );
    }

    #[test]
    fn empty_text_emits_nothing() {
        assert!(SocketIoPayload::parse(None, "   ").args.is_empty());
    }
}
