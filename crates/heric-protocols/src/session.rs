//! Long-lived sessions: WebSocket, Socket.IO, gRPC streams and event streams.

use heric_model::{Execution, Id, TimelineEntry};

/// Which protocol a document speaks.
///
/// Derived from the document's [`heric_model::DocKind`] plus, for HTTP
/// documents, whether a header makes them an event stream. The original decides
/// an event stream by an exact first `Accept: text/event-stream` header
/// (`PRO-044`), and Heric keeps that rule rather than adding a model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Plain HTTP, one request one response.
    Http,
    /// A GraphQL request over HTTP.
    GraphQl,
    /// A WebSocket.
    WebSocket,
    /// A Socket.IO session.
    SocketIo,
    /// gRPC, any of the four method kinds.
    Grpc,
    /// Server-sent events over HTTP.
    EventStream,
}

/// Identifies a live session.
///
/// Not the document id: the original keeps one connection per request but can
/// hold several streams open at once, and the id must survive a pane being
/// unmounted and remounted (`PRO-034`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub Id);

/// What a session reports.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// The connection is open.
    Opened {
        /// The handshake's status, 101 for a WebSocket.
        status: u16,
        /// The handshake's headers.
        headers: Vec<(String, String)>,
    },
    /// A message arrived.
    Message {
        /// Decoded text when the payload is text, raw bytes otherwise.
        payload: MessagePayload,
        /// When it arrived, for the log's ordering and duration display.
        at: std::time::Duration,
    },
    /// Bytes went out.
    Sent {
        /// How many bytes.
        bytes: usize,
    },
    /// The session closed, cleanly or not.
    Closed(CloseInfo),
    /// The session failed.
    Failed(String),
    /// A log line for the session's own timeline.
    Log(TimelineEntry),
}

/// A message payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePayload {
    /// UTF-8 text.
    Text(String),
    /// A binary frame.
    Binary(Vec<u8>),
}

impl MessagePayload {
    /// The payload as text, lossily for binary frames.
    #[must_use]
    pub fn as_text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Self::Text(text) => std::borrow::Cow::Borrowed(text),
            Self::Binary(bytes) => String::from_utf8_lossy(bytes),
        }
    }
}

/// Why a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseInfo {
    /// The close code, `1005` when the peer sent none.
    pub code: u16,
    /// The reason, empty when absent.
    pub reason: String,
    /// Whether the close was clean. The original distinguishes `wasClean` and
    /// shows an unclean 1005 rather than hiding it (`PRO-038`).
    pub was_clean: bool,
}

impl CloseInfo {
    /// A close with no code and no reason.
    #[must_use]
    pub fn unclean() -> Self {
        Self {
            code: 1005,
            reason: String::new(),
            was_clean: false,
        }
    }
}

/// Drives a live session.
///
/// One object per open connection, owned by the runtime and driven from a task.
/// Cancellation is by [`Session::close`], not by dropping, because a dropped
/// socket leaves the peer's side open until it times out.
pub trait Session: Send {
    /// The session's id.
    fn id(&self) -> &SessionId;

    /// The protocol, for the UI's pane selection.
    fn protocol(&self) -> Protocol;

    /// Sends one message.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError::Closed`] when the session is gone.
    fn send(&mut self, payload: &MessagePayload) -> Result<(), crate::ProtocolError>;

    /// Blocking receive of the next event.
    ///
    /// Blocking because the caller runs it on a blocking-pool thread; the GUI
    /// never calls it directly.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError::Closed`] once the session has ended.
    fn next(&mut self) -> Result<SessionEvent, crate::ProtocolError>;

    /// Closes the session, sending a close frame where the protocol has one.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError`] when the close could not be delivered.
    /// The session is closed either way.
    fn close(self: Box<Self>) -> Result<(), crate::ProtocolError>;

    /// The state a UI shows in the sidebar.
    fn state(&self) -> crate::ReadyState;
}

/// A session creator.
///
/// Separate from [`Session`] so the caller can build a session without owning
/// the transport configuration.
pub trait SessionHost: Send + Sync {
    /// Opens a session.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError`] when the connection fails. A handshake
    /// answered with a non-101 status becomes
    /// [`crate::ProtocolError::Handshake`], which the GUI renders as an
    /// inspectable response like any other.
    fn open(
        &self,
        request: &heric_model::PreparedRequest,
    ) -> Result<Box<dyn Session>, crate::ProtocolError>;

    /// The response the session's handshake produced, once it is known.
    ///
    /// The original keeps the handshake response as history so a failed upgrade
    /// can be inspected. Returning it separately keeps [`Session`] free of
    /// history concerns.
    fn handshake(&self) -> Option<Execution>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unclean_close_reports_1005() {
        let info = CloseInfo::unclean();
        assert_eq!(info.code, 1005);
        assert!(!info.was_clean);
    }

    #[test]
    fn a_binary_payload_still_has_text() {
        let payload = MessagePayload::Binary(b"hello".to_vec());
        assert_eq!(payload.as_text().as_ref(), "hello");
    }
}
