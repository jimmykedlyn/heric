//! Non-HTTP protocols: GraphQL, gRPC, WebSocket, Socket.IO and event streams.
//!
//! Every protocol here has a request half and a session half. A one-shot request
//! (a GraphQL query, a gRPC unary call) is executed and returns an
//! [`Execution`]. A long-lived session (a WebSocket, a gRPC stream, an event
//! stream) is opened and then driven by [`SessionEvent`]s until it closes.
//!
//! This crate owns no transport of its own for HTTP-shaped work: GraphQL
//! introspection and event streams ride the `heric_http::HttpEngine` through
//! the [`ProtocolHost`] port, which is what lets the GUI run one connection pool
//! and one cookie jar for everything.

#![deny(unsafe_code)]

mod error;
mod graphql;
mod grpc;
mod session;
mod socketio;
mod sse;
mod websocket;

pub use error::{ProtocolDiagnostic, ProtocolError};
pub use graphql::{GraphQlOperation, GraphQlPayload, IntrospectionQuery};
pub use grpc::{
    CallKind, Descriptor, DescriptorSet, GrpcStatus, GrpcTarget, MethodInfo, ProtoSource,
};
pub use session::{
    CloseInfo, MessagePayload, Protocol, Session, SessionEvent, SessionHost, SessionId,
};
pub use socketio::{SocketIoConfig, SocketIoPayload};
pub use sse::{SseEvent, SseReconnect};
pub use websocket::{ReadyState, WebSocketRequest};

use heric_model::{Execution, PreparedRequest};

/// Returns the version of `heric-protocols`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The services a protocol needs from the rest of the application.
///
/// A port rather than direct dependencies, so a protocol that needs to send a
/// sub-request (GraphQL introspection) or share a cookie jar does not pull in
/// the whole HTTP stack and can be tested with a fake.
pub trait ProtocolHost: Send + Sync {
    /// Sends an HTTP request built by a protocol.
    ///
    /// Used by GraphQL introspection, which is deliberately a private request
    /// sharing the original's credentials and proxy (`PRO-003`).
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::Transport`] when the engine fails.
    fn send(&self, request: &PreparedRequest) -> Result<Execution, ProtocolError>;

    /// Whether a path is inside the collection's data folders.
    ///
    /// gRPC loads proto files from disk, so the same allowlist that guards
    /// multipart uploads and `{% file %}` applies (`HTTP-006`).
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::PathNotAllowed`] when it is not.
    fn check_path(&self, path: &std::path::Path, purpose: &str) -> Result<(), ProtocolError>;

    /// Registers a value that must be masked in logs and timelines.
    fn redact(&self, label: &str, value: &str);
}

/// Runs a one-shot non-streaming protocol request to completion.
///
/// GraphQL and unary gRPC go through here. Streaming calls open a
/// [`Session`] instead.
pub trait ProtocolRequest: Send + Sync {
    /// Executes the request.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the request cannot be executed. A protocol
    /// error reported by the server is an [`Execution`] with a status, not an
    /// error: the original keeps it in history and shows it in the pane.
    fn execute(
        &self,
        request: &PreparedRequest,
        host: &dyn ProtocolHost,
    ) -> Result<Execution, ProtocolError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
