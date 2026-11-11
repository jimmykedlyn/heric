//! Wire types passed between the request pipeline's crates.
//!
//! These live in `heric-model` rather than in `heric-http` because both
//! `heric-template` (`TagHost::send`) and `heric-auth` (`AuthInput`) name them,
//! and neither may depend on `heric-http` without creating a cycle. They
//! describe bytes on the wire, not persistence, so they belong with the shared
//! vocabulary.

use std::time::Duration;

use crate::{DocKind, Id};

/// One HTTP header.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Header {
    /// Header name, as the user typed it.
    pub name: String,
    /// Header value, already rendered.
    pub value: String,
}

impl Header {
    /// Creates a header.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// A request after every templated field has been rendered.
///
/// The engine never sees a template: rendering happens before this type is
/// built, which is what keeps `heric-http` independent of `heric-template`.
/// Which request and which method produced it is kept for the timeline and for
/// the `{% response %}` cache key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    /// The request document this was rendered from.
    pub request_id: Id,
    /// The request kind, which decides how the result is interpreted.
    pub kind: DocKind,
    /// Method as it goes on the wire.
    pub method: String,
    /// Fully rendered URL.
    pub url: String,
    /// Rendered headers.
    pub headers: Vec<Header>,
    /// The body, already built.
    pub body: Body,
}

/// A request body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Body {
    /// No body.
    #[default]
    Empty,
    /// Raw bytes with an optional content type.
    Bytes {
        /// The bytes to send.
        bytes: Vec<u8>,
        /// Value for `Content-Type`, when already known.
        content_type: Option<String>,
    },
    /// A file streamed from disk, so a large upload is not read into memory.
    File {
        /// Path to stream.
        path: std::path::PathBuf,
        /// Value for `Content-Type`, when already known.
        content_type: Option<String>,
    },
}

impl Body {
    /// A body made of UTF-8 text.
    #[must_use]
    pub fn text(text: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self::Bytes {
            bytes: text.into().into_bytes(),
            content_type: Some(content_type.into()),
        }
    }

    /// The content type this body wants, if any.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Bytes { content_type, .. } | Self::File { content_type, .. } => {
                content_type.as_deref()
            }
        }
    }
}

/// Which kind of event a timeline entry records.
///
/// These map onto libcurl's debug callback types, plus a text channel for
/// engine-level notes such as OAuth token failures. The persisted Console
/// timeline is built from these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimelineKind {
    /// Request headers leaving the client.
    HeaderOut,
    /// Response headers arriving.
    HeaderIn,
    /// Request body bytes leaving.
    DataOut,
    /// Response body bytes arriving.
    DataIn,
    /// TLS handshake data.
    SslDataIn,
    /// TLS handshake data leaving.
    SslDataOut,
    /// A note from the engine rather than from libcurl.
    Text,
}

/// One entry in the persisted Console timeline.
///
/// Redaction happens in the sink that writes these, not in the engine that
/// produces them: the in-memory timeline is complete for the UI and the on-disk
/// one is safe.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimelineEntry {
    /// Monotonic offset from the start of the execution.
    pub at: Duration,
    /// What sort of event this is.
    pub kind: TimelineKind,
    /// The payload, already stringified.
    pub text: String,
}

/// Per-phase timings, in the granularity libcurl exposes.
///
/// The `curl` crate binds only the `f64`-second counters, not the microsecond
/// `_TIME_T` ones, so these are phase-accurate rather than bit-identical to the
/// original. The waterfall shows phases; tests compare ordering, not values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Timings {
    /// Name resolution.
    pub name_lookup: Duration,
    /// TCP connect.
    pub connect: Duration,
    /// TLS handshake.
    pub app_connect: Duration,
    /// From the start until just before the transfer begins.
    pub pre_transfer: Duration,
    /// Until the first byte of the body arrives.
    pub start_transfer: Duration,
    /// Time spent following redirects.
    pub redirect: Duration,
    /// Total.
    pub total: Duration,
}

/// The outcome of executing a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    /// Status code, or `0` when the transport failed.
    pub status: u16,
    /// Reason phrase as received.
    pub status_message: String,
    /// HTTP version as reported by the engine.
    pub http_version: String,
    /// Final URL after redirects.
    pub url: String,
    /// Response headers.
    pub headers: Vec<Header>,
    /// The body.
    pub body: Vec<u8>,
    /// Content type as received.
    pub content_type: Option<String>,
    /// Bytes read off the socket.
    pub bytes_read: u64,
    /// Bytes of body.
    pub bytes_content: i64,
    /// Wall-clock duration.
    pub elapsed: Duration,
    /// Per-phase timings.
    pub timings: Timings,
    /// The Console timeline.
    pub timeline: Vec<TimelineEntry>,
    /// Transport error, empty when the exchange completed.
    ///
    /// An error is carried in the execution rather than returned instead of
    /// it, because the original keeps a failed response in history with its
    /// partial timeline.
    pub error: Option<String>,
}

impl Execution {
    /// Convenience for tests and for fakes.
    #[must_use]
    pub fn ok(status: u16) -> Self {
        Self {
            status,
            status_message: String::new(),
            http_version: "HTTP/1.1".to_owned(),
            url: String::new(),
            headers: Vec::new(),
            body: Vec::new(),
            content_type: None,
            bytes_read: 0,
            bytes_content: 0,
            elapsed: Duration::ZERO,
            timings: Timings::default(),
            timeline: Vec::new(),
            error: None,
        }
    }

    /// Whether the transport completed, regardless of status code.
    #[must_use]
    pub fn is_transport_ok(&self) -> bool {
        self.error.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_body_has_no_content_type() {
        assert_eq!(Body::Empty.content_type(), None);
        assert_eq!(Body::default(), Body::Empty);
    }

    #[test]
    fn a_text_body_reports_its_content_type() {
        let body = Body::text("{}", "application/json");
        assert_eq!(body.content_type(), Some("application/json"));
    }

    #[test]
    fn a_constructed_execution_is_transport_ok() {
        let exec = Execution::ok(204);
        assert!(exec.is_transport_ok());
        assert_eq!(exec.status, 204);
    }

    #[test]
    fn timings_default_to_zero() {
        assert_eq!(Timings::default().total, Duration::ZERO);
    }
}
