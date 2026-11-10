//! Non-HTTP protocols for Heric.
//!
//! gRPC (tonic + prost-reflect with proto files and server reflection),
//! WebSocket, Server-Sent Events and GraphQL (introspection and schema).
//! See `docs/inventory/protocols.md` once the inventory lands.

/// Protocol kinds handled by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// gRPC over HTTP/2.
    Grpc,
    /// WebSocket connections.
    WebSocket,
    /// Server-Sent Events streams.
    Sse,
    /// GraphQL over HTTP.
    GraphQL,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocols_are_distinguishable() {
        assert_ne!(Protocol::Grpc, Protocol::GraphQL);
    }
}
