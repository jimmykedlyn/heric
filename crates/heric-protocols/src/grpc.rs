//! gRPC: targets, descriptors, method kinds and status codes.

use std::path::PathBuf;

/// A gRPC target, parsed from the request's URL form.
///
/// The original accepts `grpc:` and `grpcs:` plus a `unix:` form, and defaults a
/// bare host to `grpc://`. Keeping the parsed form lets the send path avoid
/// re-deriving the scheme from a string at the point where a mistake would
/// connect over plaintext to a port expecting TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcTarget {
    /// Host and port, or a socket path for a Unix socket.
    pub endpoint: Endpoint,
    /// Whether the channel uses TLS.
    pub tls: bool,
    /// Method path, `package.Service/Method`, when the URL carries one.
    pub method: Option<String>,
}

/// Where a gRPC channel connects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// A `host:port` endpoint.
    HostPort(String),
    /// A Unix domain socket.
    Unix(PathBuf),
}

impl GrpcTarget {
    /// Parses a gRPC URL.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError::InvalidUrl`] when the scheme is not one of
    /// the three accepted forms.
    pub fn parse(url: &str) -> Result<Self, crate::ProtocolError> {
        let (scheme, rest) =
            url.split_once(':')
                .ok_or_else(|| crate::ProtocolError::InvalidUrl {
                    protocol: "gRPC",
                    detail: "no scheme".to_owned(),
                })?;
        let rest = rest.trim_start_matches("//");
        // A Unix socket's path is the whole remainder, leading slash included:
        // splitting on the first `/` would yield an empty authority and lose
        // the path. A method may still follow the socket file, as in
        // `unix:///var/run/app.sock/pkg.Svc/Method`.
        let (endpoint, tls, method) = match scheme {
            "unix" => {
                let (path, method) = match rest.split_once(".sock/") {
                    Some((path, tail)) if !tail.is_empty() => {
                        (format!("{path}.sock"), Some(tail.to_owned()))
                    }
                    _ => (rest.to_owned(), None),
                };
                (Endpoint::Unix(PathBuf::from(path)), false, method)
            }
            "grpc" | "grpcs" => {
                let (authority, method) = match rest.split_once('/') {
                    Some((authority, tail)) if !tail.is_empty() => {
                        (authority, Some(tail.to_owned()))
                    }
                    _ => (rest, None),
                };
                (
                    Endpoint::HostPort(authority.to_owned()),
                    scheme == "grpcs",
                    method,
                )
            }
            other => {
                return Err(crate::ProtocolError::InvalidUrl {
                    protocol: "gRPC",
                    detail: format!("unsupported scheme `{other}`"),
                });
            }
        };
        Ok(Self {
            endpoint,
            tls,
            method,
        })
    }
}

/// The four gRPC method kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallKind {
    /// One request, one response.
    #[default]
    Unary,
    /// Many requests, one response.
    ClientStreaming,
    /// One request, many responses.
    ServerStreaming,
    /// Many requests, many responses.
    BidiStreaming,
}

impl CallKind {
    /// Whether the caller sends more than one message.
    #[must_use]
    pub fn client_streams(self) -> bool {
        matches!(self, Self::ClientStreaming | Self::BidiStreaming)
    }

    /// Whether the server sends more than one message.
    #[must_use]
    pub fn server_streams(self) -> bool {
        matches!(self, Self::ServerStreaming | Self::BidiStreaming)
    }
}

/// One method discovered in a proto file or through reflection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo {
    /// Fully qualified path, `package.Service/Method`.
    pub path: String,
    /// The kind, derived from the proto's `stream` markers.
    pub kind: CallKind,
    /// Whether the input is a stream.
    pub input_stream: bool,
    /// Whether the output is a stream.
    pub output_stream: bool,
}

/// A decoded proto descriptor set.
///
/// The original hands descriptors to a JavaScript reflection library. Heric
/// decodes them into `prost-reflect` types at load time, which is why the port
/// returns descriptors rather than a file path: the send path never re-reads
/// disk.
#[derive(Debug, Clone, Default)]
pub struct DescriptorSet {
    /// Every message, enum and service, already resolved.
    pub descriptors: Option<prost_reflect::DescriptorPool>,
    /// The services and methods, sorted by path for a stable menu.
    pub methods: Vec<MethodInfo>,
}

/// One descriptor blob, kept whole for the UI's schema tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    /// The file it came from, relative to the collection when inside it.
    pub source: ProtoSource,
    /// The encoded `FileDescriptorProto` bytes.
    pub bytes: Vec<u8>,
}

/// Where proto material came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtoSource {
    /// A file in the collection's proto directory tree.
    File(PathBuf),
    /// A directory imported recursively.
    Directory(PathBuf),
    /// Server reflection.
    Reflection(String),
}

/// A gRPC status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcStatus {
    /// The numeric code, `0` being `OK`.
    pub code: i32,
    /// The human-readable message.
    pub message: String,
    /// Trailing metadata.
    pub metadata: Vec<(String, String)>,
}

impl GrpcStatus {
    /// Whether the call succeeded. The original suppresses the `CANCELLED`
    /// status in the UI when the user cancelled it, so this is only about the
    /// wire.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.code == 0
    }

    /// The status name, for the tag shown next to the response.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.code {
            0 => "OK",
            1 => "CANCELLED",
            2 => "UNKNOWN",
            3 => "INVALID_ARGUMENT",
            4 => "DEADLINE_EXCEEDED",
            5 => "NOT_FOUND",
            6 => "ALREADY_EXISTS",
            7 => "PERMISSION_DENIED",
            8 => "RESOURCE_EXHAUSTED",
            9 => "FAILED_PRECONDITION",
            10 => "ABORTED",
            11 => "OUT_OF_RANGE",
            12 => "UNIMPLEMENTED",
            13 => "INTERNAL",
            14 => "UNAVAILABLE",
            15 => "DATA_LOSS",
            16 => "UNAUTHENTICATED",
            _ => "UNKNOWN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_defaults_to_plaintext() -> Result<(), Box<dyn std::error::Error>> {
        let target = GrpcTarget::parse("grpc://localhost:50051/pkg.Svc/Method")?;
        assert!(!target.tls);
        assert_eq!(target.method.as_deref(), Some("pkg.Svc/Method"));
        Ok(())
    }

    #[test]
    fn grpcs_enables_tls() -> Result<(), Box<dyn std::error::Error>> {
        let target = GrpcTarget::parse("grpcs://api.example.test:443")?;
        assert!(target.tls);
        assert_eq!(
            target.endpoint,
            Endpoint::HostPort("api.example.test:443".to_owned())
        );
        assert_eq!(target.method, None);
        Ok(())
    }

    #[test]
    fn a_unix_socket_is_its_own_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let target = GrpcTarget::parse("unix:///var/run/app.sock")?;
        assert_eq!(
            target.endpoint,
            Endpoint::Unix(PathBuf::from("/var/run/app.sock"))
        );
        assert!(!target.tls);
        Ok(())
    }

    #[test]
    fn an_unknown_scheme_is_rejected() {
        assert!(matches!(
            GrpcTarget::parse("http://localhost:50051"),
            Err(crate::ProtocolError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn method_kinds_report_their_streaming_sides() {
        assert!(CallKind::BidiStreaming.client_streams());
        assert!(CallKind::BidiStreaming.server_streams());
        assert!(!CallKind::Unary.client_streams());
        assert!(CallKind::ServerStreaming.server_streams());
        assert!(!CallKind::ServerStreaming.client_streams());
    }

    #[test]
    fn status_names_cover_the_standard_codes() {
        let ok = GrpcStatus {
            code: 0,
            message: String::new(),
            metadata: Vec::new(),
        };
        assert!(ok.is_ok());
        assert_eq!(ok.name(), "OK");
        let unimplemented = GrpcStatus { code: 12, ..ok };
        assert_eq!(unimplemented.name(), "UNIMPLEMENTED");
        assert!(!unimplemented.is_ok());
    }
}
