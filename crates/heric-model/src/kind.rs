//! The 44 entity types the original stores, with their wire names and id
//! prefixes.

/// One of the 44 entity types the original client stores.
///
/// The variants keep the original type names verbatim, because those names
/// appear in the files Heric reads and writes. `as_str` returns the wire form,
/// which is what NeDB documents, YAML exports and Git collections carry.
///
/// The list is generated from `docs/inventory/models.md` and is exhaustive by
/// construction: a new type must be added there first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DocKind {
    /// An OpenAPI specification attached to a workspace. Prefix `spc`.
    ApiSpec,
    /// A workspace-level CA certificate record. Prefix `crt`.
    CaCertificate,
    /// A client certificate bound to a host. Prefix `crt`, shared with
    /// [`DocKind::CaCertificate`] in the original.
    ClientCertificate,
    /// Provider credentials for a cloud API. Prefix `cloudCred`.
    CloudCredential,
    /// A cookie jar belonging to a workspace. Prefix `jar`.
    CookieJar,
    /// A base or sub environment. Prefix `env`.
    Environment,
    /// Git credentials for one provider. Prefix `git_creds`.
    GitCredentials,
    /// A Git repository attached to a workspace. Prefix `git`.
    GitRepository,
    /// A gRPC request. Prefix `greq`.
    GrpcRequest,
    /// UI state for a gRPC request. Prefix `greqm`.
    GrpcRequestMeta,
    /// A route served by a mock server. Prefix `mock-route`.
    MockRoute,
    /// A mock server. Prefix `mock`.
    MockServer,
    /// A stored MCP payload. Prefix `mcp-payload`.
    McpPayload,
    /// An MCP request. Prefix `mcp-req`.
    McpRequest,
    /// An MCP response. Prefix `mcp-response`.
    McpResponse,
    /// A cached OAuth 2.0 token set. Prefix `oa2`.
    OAuth2Token,
    /// Key-value data owned by a plugin. Prefix `plg`.
    PluginData,
    /// A project, the container above workspaces. Prefix `proj`.
    Project,
    /// Lint rules for Design mode. Prefix `plr`.
    ProjectLintRuleset,
    /// A directory of proto files. Prefix `pd`.
    ProtoDirectory,
    /// A proto file. Prefix `pf`.
    ProtoFile,
    /// An HTTP request. Prefix `req`.
    Request,
    /// A folder grouping requests. Prefix `fld`.
    RequestGroup,
    /// UI state for a folder. Prefix `fldm`.
    RequestGroupMeta,
    /// UI state for an HTTP request. Prefix `reqm`.
    RequestMeta,
    /// A snapshot of a request at send time. Prefix `rvr`.
    RequestVersion,
    /// An HTTP response. Prefix `res`.
    Response,
    /// A collection runner test result. Prefix `rtr`.
    RunnerTestResult,
    /// Application settings, a singleton. Prefix `set`.
    Settings,
    /// A stored Socket.IO payload. Prefix `socket-io-payload`.
    SocketIOPayload,
    /// A Socket.IO request. Prefix `socketio-req`.
    SocketIORequest,
    /// UI state for a Socket.IO request. Prefix `socketio-req-meta`.
    SocketIORequestMeta,
    /// A Socket.IO response. Prefix `socketIO-res`.
    SocketIOResponse,
    /// Launch and usage counters, a singleton. Prefix `sta`.
    Stats,
    /// A generated unit test. Prefix `ut`.
    UnitTest,
    /// A unit test run result. Prefix `utr`.
    UnitTestResult,
    /// A unit test suite. Prefix `uts`.
    UnitTestSuite,
    /// A user session, local parts only. Prefix `usr`.
    UserSession,
    /// A stored WebSocket payload. Prefix `ws-payload`.
    WebSocketPayload,
    /// A WebSocket request. Prefix `ws-req`.
    WebSocketRequest,
    /// UI state for a WebSocket request. Prefix `ws-req-meta`.
    WebSocketRequestMeta,
    /// A WebSocket response. Prefix `ws-res`.
    WebSocketResponse,
    /// A workspace. Prefix `wrk`.
    Workspace,
    /// UI state for a workspace. Prefix `wrkm`.
    WorkspaceMeta,
}

macro_rules! kinds {
    ($( $variant:ident => ($wire:literal, $prefix:literal) ),* $(,)?) => {
        impl DocKind {
            /// Every kind, in the order the inventory lists them.
            pub const ALL: &'static [DocKind] = &[$(DocKind::$variant),*];

            /// The wire name, which is the value of the `type` field and the
            /// suffix of the NeDB file name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(DocKind::$variant => $wire),* }
            }

            /// The id prefix the original uses when it generates an id of this
            /// kind.
            ///
            /// Two kinds share a prefix: [`DocKind::CaCertificate`] and
            /// [`DocKind::ClientCertificate`] both use `crt`, so a prefix does
            /// not identify a kind on its own.
            #[must_use]
            pub const fn prefix(self) -> &'static str {
                match self { $(DocKind::$variant => $prefix),* }
            }

            /// Parses a wire name.
            ///
            /// # Errors
            ///
            /// Returns [`crate::ModelError::UnknownKind`] when the name is not
            /// one of the 44, which is what a file written by a newer version
            /// looks like. The caller decides whether to skip the document or
            /// fail the read.
            pub fn parse(wire: &str) -> Result<Self, crate::ModelError> {
                match wire {
                    $($wire => Ok(DocKind::$variant),)*
                    other => Err(crate::ModelError::UnknownKind(other.to_owned())),
                }
            }
        }
    };
}

kinds! {
    ApiSpec => ("ApiSpec", "spc"),
    CaCertificate => ("CaCertificate", "crt"),
    ClientCertificate => ("ClientCertificate", "crt"),
    CloudCredential => ("CloudCredential", "cloudCred"),
    CookieJar => ("CookieJar", "jar"),
    Environment => ("Environment", "env"),
    GitCredentials => ("GitCredentials", "git_creds"),
    GitRepository => ("GitRepository", "git"),
    GrpcRequest => ("GrpcRequest", "greq"),
    GrpcRequestMeta => ("GrpcRequestMeta", "greqm"),
    MockRoute => ("MockRoute", "mock-route"),
    MockServer => ("MockServer", "mock"),
    McpPayload => ("McpPayload", "mcp-payload"),
    McpRequest => ("McpRequest", "mcp-req"),
    McpResponse => ("McpResponse", "mcp-response"),
    OAuth2Token => ("OAuth2Token", "oa2"),
    PluginData => ("PluginData", "plg"),
    Project => ("Project", "proj"),
    ProjectLintRuleset => ("ProjectLintRuleset", "plr"),
    ProtoDirectory => ("ProtoDirectory", "pd"),
    ProtoFile => ("ProtoFile", "pf"),
    Request => ("Request", "req"),
    RequestGroup => ("RequestGroup", "fld"),
    RequestGroupMeta => ("RequestGroupMeta", "fldm"),
    RequestMeta => ("RequestMeta", "reqm"),
    RequestVersion => ("RequestVersion", "rvr"),
    Response => ("Response", "res"),
    RunnerTestResult => ("RunnerTestResult", "rtr"),
    Settings => ("Settings", "set"),
    SocketIOPayload => ("SocketIOPayload", "socket-io-payload"),
    SocketIORequest => ("SocketIORequest", "socketio-req"),
    SocketIORequestMeta => ("SocketIORequestMeta", "socketio-req-meta"),
    SocketIOResponse => ("SocketIOResponse", "socketIO-res"),
    Stats => ("Stats", "sta"),
    UnitTest => ("UnitTest", "ut"),
    UnitTestResult => ("UnitTestResult", "utr"),
    UnitTestSuite => ("UnitTestSuite", "uts"),
    UserSession => ("UserSession", "usr"),
    WebSocketPayload => ("WebSocketPayload", "ws-payload"),
    WebSocketRequest => ("WebSocketRequest", "ws-req"),
    WebSocketRequestMeta => ("WebSocketRequestMeta", "ws-req-meta"),
    WebSocketResponse => ("WebSocketResponse", "ws-res"),
    Workspace => ("Workspace", "wrk"),
    WorkspaceMeta => ("WorkspaceMeta", "wrkm"),
}

impl std::fmt::Display for DocKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for DocKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for DocKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_forty_four_kinds() {
        assert_eq!(DocKind::ALL.len(), 44);
    }

    #[test]
    fn wire_names_are_unique() {
        let mut names: Vec<&str> = DocKind::ALL.iter().map(|k| k.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn every_kind_round_trips_through_its_wire_name() -> Result<(), crate::ModelError> {
        for kind in DocKind::ALL {
            assert_eq!(DocKind::parse(kind.as_str())?, *kind);
        }
        Ok(())
    }

    #[test]
    fn the_two_certificate_kinds_share_a_prefix() {
        assert_eq!(
            DocKind::CaCertificate.prefix(),
            DocKind::ClientCertificate.prefix()
        );
    }

    #[test]
    fn unknown_kind_is_an_error() {
        assert!(DocKind::parse("NotAType").is_err());
    }
}
