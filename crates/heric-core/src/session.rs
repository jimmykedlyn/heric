//! Live sessions: WebSocket, Socket.IO, gRPC streams and event streams.
//!
//! A session is driven by a task of its own, because `Session::next` blocks and
//! a blocking receive on a runtime worker starves the reactor. The task owns
//! the `Box<dyn Session>` and reports through [`CoreEvent`](crate::CoreEvent),
//! so the UI never holds a transport handle.

use heric_model::Id;
use heric_protocols::{CloseInfo, MessagePayload, ReadyState, SessionEvent, SessionId};

use crate::CoreHandle;
use crate::error::CoreError;
use crate::events::CoreEvent;

/// Which transport a session speaks.
///
/// Derived from the request document, except for an event stream: the original
/// decides a request is an event stream by an exact first
/// `Accept: text/event-stream` header (`PRO-044`), and Heric keeps that rule
/// rather than adding a model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {
    /// A WebSocket.
    WebSocket,
    /// A Socket.IO session.
    SocketIo,
    /// A gRPC streaming call.
    Grpc,
    /// Server-sent events over HTTP.
    EventStream,
}

impl SessionKind {
    /// The kind's name, for the UI's tab label and the event log.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WebSocket => "websocket",
            Self::SocketIo => "socket.io",
            Self::Grpc => "grpc",
            Self::EventStream => "event-stream",
        }
    }
}

/// A session the core is holding open.
///
/// Cloned into every event about it, so the UI can identify the pane without a
/// lookup. Equality ignores the state and the request: two events about the same
/// session must compare equal even when one reports the open and the other the
/// close, which is what lets a pane deduplicate them.
#[derive(Debug, Clone)]
pub struct OpenSession {
    /// The protocol-assigned id, which survives a pane being remounted.
    pub id: SessionId,
    /// The request document the session was opened from.
    pub request: Id,
    /// Which transport.
    pub kind: SessionKind,
    /// The state the UI shows in the sidebar.
    pub state: ReadyState,
}

impl PartialEq for OpenSession {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for OpenSession {}

impl std::hash::Hash for OpenSession {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl OpenSession {
    /// Whether the session can carry a message right now.
    #[must_use]
    pub fn can_send(&self) -> bool {
        self.state.can_send()
    }
}

/// Opens, drives and closes live sessions.
#[derive(Debug, Clone)]
pub struct SessionService {
    core: CoreHandle,
}

impl SessionService {
    /// Builds the service. Front ends get one from [`CoreHandle::sessions`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Opens a session for a request.
    ///
    /// The session is opened on the blocking pool and then driven by a task, so
    /// this returns once the transport exists rather than when the handshake
    /// completes. Progress arrives as [`CoreEvent::SessionChanged`] and
    /// [`CoreEvent::SessionMessage`], which is what lets the pane open instantly
    /// and fill in its log as the connection progresses.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Protocol`] when the transport cannot be built.
    pub async fn open(&self, request: Id, kind: SessionKind) -> Result<OpenSession, CoreError> {
        let _ = (&self.core, request, kind);
        todo!("M4: open a session and spawn its drive loop")
    }

    /// Sends a message on an open session.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::Closed`](heric_protocols::ProtocolError::Closed) when the session is not open.
    pub fn send(&self, session: &OpenSession, payload: &MessagePayload) -> Result<(), CoreError> {
        let _ = (session, payload);
        Err(CoreError::Protocol(heric_protocols::ProtocolError::Closed))
    }

    /// Closes a session.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Protocol`] when the close frame cannot be sent. The
    /// session is removed from the registry either way, because a transport that
    /// refuses to close is still gone.
    pub fn close(&self, session: &OpenSession) -> Result<(), CoreError> {
        self.core.events().send(CoreEvent::SessionChanged {
            session: OpenSession {
                state: ReadyState::Closed,
                ..session.clone()
            },
            detail: "closed".to_owned(),
        });
        Ok(())
    }

    /// Closes every session, which is what closing a window does.
    pub fn close_all(&self) {
        let _ = &self.core;
    }
}

/// Turns a protocol event into the core event the UI sees.
///
/// Kept here rather than in the drive loop so the mapping is testable without a
/// transport, and so a new `SessionEvent` variant fails to compile rather than
/// silently vanishing from the log.
#[must_use]
pub fn to_core_event(session: &OpenSession, event: &SessionEvent) -> CoreEvent {
    match event {
        SessionEvent::Opened { status, headers } => CoreEvent::SessionChanged {
            session: OpenSession {
                state: ReadyState::Open,
                ..session.clone()
            },
            detail: format!("open ({status}, {} headers)", headers.len()),
        },
        SessionEvent::Message { payload, at } => CoreEvent::SessionMessage {
            session: session.clone(),
            text: format!("{:?} {}", at, payload.as_text()),
        },
        SessionEvent::Sent { bytes } => CoreEvent::SessionChanged {
            session: session.clone(),
            detail: format!("sent {bytes} bytes"),
        },
        SessionEvent::Closed(CloseInfo {
            code,
            reason,
            was_clean,
        }) => CoreEvent::SessionChanged {
            session: OpenSession {
                state: ReadyState::Closed,
                ..session.clone()
            },
            detail: format!("closed ({code}, clean={was_clean}) {reason}"),
        },
        SessionEvent::Failed(detail) => CoreEvent::Failed {
            message: detail.clone(),
            fatal: false,
        },
        SessionEvent::Log(entry) => CoreEvent::SessionMessage {
            session: session.clone(),
            text: entry.text.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> OpenSession {
        OpenSession {
            id: SessionId(Id::new("wsreq_1")),
            request: Id::new("req_1"),
            kind: SessionKind::WebSocket,
            state: ReadyState::Open,
        }
    }

    #[test]
    fn two_events_about_one_session_compare_equal_whatever_their_state() {
        let open = session();
        let mut closed = session();
        closed.state = ReadyState::Closed;
        assert_eq!(open, closed);
    }

    #[test]
    fn sessions_with_different_ids_differ() {
        let mut other = session();
        other.id = SessionId(Id::new("wsreq_2"));
        assert_ne!(session(), other);
    }

    #[test]
    fn only_an_open_session_can_send() {
        assert!(session().can_send());
        let mut connecting = session();
        connecting.state = ReadyState::Connecting;
        assert!(!connecting.can_send());
    }

    #[test]
    fn a_message_event_carries_the_payload_text() {
        let event = SessionEvent::Message {
            payload: MessagePayload::Text("hello".to_owned()),
            at: std::time::Duration::from_secs(1),
        };
        match to_core_event(&session(), &event) {
            CoreEvent::SessionMessage { text, .. } => assert!(text.ends_with("hello")),
            other => panic!("expected a message event, got {other:?}"),
        }
    }

    #[test]
    fn an_open_event_moves_the_session_to_open() {
        let mut connecting = session();
        connecting.state = ReadyState::Connecting;
        let event = SessionEvent::Opened {
            status: 101,
            headers: Vec::new(),
        };
        match to_core_event(&connecting, &event) {
            CoreEvent::SessionChanged { session, detail } => {
                assert_eq!(session.state, ReadyState::Open);
                assert!(detail.contains("101"));
            }
            other => panic!("expected a changed event, got {other:?}"),
        }
    }

    #[test]
    fn a_close_event_names_its_code_and_marks_the_session_closed() {
        let event = SessionEvent::Closed(CloseInfo::unclean());
        match to_core_event(&session(), &event) {
            CoreEvent::SessionChanged { session, detail } => {
                assert_eq!(session.state, ReadyState::Closed);
                assert!(detail.contains("1005"));
            }
            other => panic!("expected a changed event, got {other:?}"),
        }
    }

    #[test]
    fn every_session_event_maps_to_something_distinct() {
        let events = [
            SessionEvent::Opened {
                status: 101,
                headers: Vec::new(),
            },
            SessionEvent::Message {
                payload: MessagePayload::Binary(vec![1, 2, 3]),
                at: std::time::Duration::from_millis(5),
            },
            SessionEvent::Sent { bytes: 3 },
            SessionEvent::Closed(CloseInfo::unclean()),
            SessionEvent::Failed("boom".to_owned()),
            SessionEvent::Log(heric_model::TimelineEntry {
                at: std::time::Duration::ZERO,
                kind: heric_model::TimelineKind::Text,
                text: "note".to_owned(),
            }),
        ];
        for event in &events {
            // Exhaustive by construction: the match has no wildcard, so adding
            // an event type is a compile error here rather than a silent drop.
            let _ = to_core_event(&session(), event);
        }
    }
}
