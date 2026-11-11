//! Execution options: timeouts, redirects, proxy and TLS.

use std::time::Duration;

use crate::{CaSource, ClientCertificate, ProxyConfig, TlsConfig};

/// How long each phase of a request may take.
///
/// Separate fields rather than one total, because the original exposes them
/// separately and a slow DNS server needs a different bound from a slow upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeout {
    /// Resolving the host.
    pub connect: Duration,
    /// Waiting for a reply once connected.
    pub response: Duration,
    /// The whole exchange.
    pub total: Duration,
}

impl Timeout {
    /// The original's defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            connect: Duration::from_secs(30),
            response: Duration::from_secs(30),
            total: Duration::ZERO,
        }
    }
}

impl Default for Timeout {
    fn default() -> Self {
        Self::defaults()
    }
}

/// What to do with a 3xx response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RedirectPolicy {
    /// Follow, up to a limit.
    #[default]
    Follow,
    /// Do not follow.
    Never,
    /// Follow, but do not send credentials to a different origin.
    SameOrigin,
}

/// Everything the engine needs besides the request itself.
#[derive(Clone, Default)]
pub struct ExecuteOptions {
    /// Phase timeouts.
    pub timeout: Timeout,
    /// Redirect handling.
    pub redirects: RedirectPolicy,
    /// Maximum redirects before failing.
    pub max_redirects: u32,
    /// Proxy resolution.
    pub proxy: ProxyConfig,
    /// Trust roots and client certificates.
    pub tls: TlsConfig,
    /// Cancellation.
    pub cancel: crate::CancelToken,
    /// Where timeline entries go. Redaction is the sink's job, so the engine
    /// hands over everything it sees.
    pub timeline: Option<std::sync::Arc<dyn crate::TimelineSink>>,
    /// Which HTTP version to request.
    pub http_version: HttpVersion,
}

impl std::fmt::Debug for ExecuteOptions {
    /// Prints every field except the sink, which has no `Debug` and whose only
    /// implementors hold buffers that would make a log line enormous.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteOptions")
            .field("timeout", &self.timeout)
            .field("redirects", &self.redirects)
            .field("max_redirects", &self.max_redirects)
            .field("proxy", &self.proxy)
            .field("tls", &self.tls)
            .field("cancel", &self.cancel)
            .field("timeline", &self.timeline.as_ref().map(|_| "sink"))
            .field("http_version", &self.http_version)
            .finish()
    }
}

/// The HTTP version to negotiate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HttpVersion {
    /// Let the engine choose.
    #[default]
    Auto,
    /// Force HTTP/1.1.
    Http1,
    /// Force HTTP/2 over TLS.
    Http2,
}

/// The default option set, plus the workspace CA when one is configured.
#[must_use]
pub fn options_with_ca(
    ca: Option<CaSource>,
    client_cert: Option<ClientCertificate>,
) -> ExecuteOptions {
    ExecuteOptions {
        tls: TlsConfig {
            native_ca: true,
            workspace_ca: ca,
            client_certificate: client_cert,
        },
        ..ExecuteOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_store_is_always_on() {
        let options = options_with_ca(None, None);
        assert!(options.tls.native_ca);
    }

    #[test]
    fn a_workspace_ca_is_additive() {
        let options = options_with_ca(
            Some(CaSource::Pem(b"-----BEGIN CERTIFICATE-----".to_vec())),
            None,
        );
        assert!(options.tls.native_ca, "native CAs must stay on");
        assert!(options.tls.workspace_ca.is_some());
    }

    #[test]
    fn redirects_are_followed_by_default() {
        assert_eq!(ExecuteOptions::default().redirects, RedirectPolicy::Follow);
    }
}
