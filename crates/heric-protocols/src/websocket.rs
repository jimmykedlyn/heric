//! WebSocket.

use heric_auth::AuthOutcome;

/// The connection state a UI shows for a realtime request.
///
/// The original broadcasts `readyState` so a connect control in the sidebar
/// survives a pane being unmounted and remounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyState {
    /// Not connected.
    Closed,
    /// The handshake is in flight.
    Connecting,
    /// Connected.
    Open,
    /// Closing.
    Closing,
}

impl ReadyState {
    /// Whether messages can be sent in this state.
    #[must_use]
    pub fn can_send(self) -> bool {
        matches!(self, Self::Open)
    }

    /// Whether a connect control should be enabled.
    #[must_use]
    pub fn can_open(self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// A WebSocket request, after rendering.
///
/// Kept separate from [`heric_model::PreparedRequest`] because a WebSocket has no
/// body and no method on the wire: the payload is sent after the connection
/// opens, the subprotocol rides a header, and authentication is limited to the
/// three shapes the original supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketRequest {
    /// The fully rendered `ws://` or `wss://` URL, with query parameters applied.
    pub url: String,
    /// Handshake headers.
    pub headers: Vec<(String, String)>,
    /// Subprotocols, taken from the `sec-websocket-protocol` header.
    pub subprotocols: Vec<String>,
    /// The payload sent once the socket opens.
    pub initial_payload: Option<String>,
    /// Authentication, already applied.
    pub auth: AuthOutcome,
    /// Whether the workspace cookie jar is sent with the handshake.
    pub send_cookies: bool,
    /// Whether cookies from the handshake are stored in the jar.
    pub store_cookies: bool,
}

impl WebSocketRequest {
    /// Whether the URL asks for TLS.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.url.starts_with("wss://")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str) -> WebSocketRequest {
        WebSocketRequest {
            url: url.to_owned(),
            headers: Vec::new(),
            subprotocols: Vec::new(),
            initial_payload: None,
            auth: AuthOutcome::default(),
            send_cookies: true,
            store_cookies: true,
        }
    }

    #[test]
    fn only_an_open_socket_accepts_a_send() {
        assert!(ReadyState::Open.can_send());
        assert!(!ReadyState::Connecting.can_send());
        assert!(!ReadyState::Closing.can_send());
    }

    #[test]
    fn only_a_closed_socket_can_be_opened() {
        assert!(ReadyState::Closed.can_open());
        assert!(!ReadyState::Open.can_open());
    }

    #[test]
    fn wss_is_secure_and_ws_is_not() {
        assert!(request("wss://example.test/socket").is_secure());
        assert!(!request("ws://example.test/socket").is_secure());
    }
}
