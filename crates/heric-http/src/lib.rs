//! The HTTP engine for Heric.
//!
//! Wraps libcurl through the `curl` crate (ADR 0004). The engine never sees a
//! template: [`heric_model::PreparedRequest`] is fully rendered before it
//! arrives, which is what keeps this crate independent of `heric-template`.
//!
//! Synchronous by contract. Every call is made from
//! `tokio::task::spawn_blocking` by `heric-core`, because libcurl is a
//! blocking C API and a blocked port must hold a blocking-pool thread rather
//! than a runtime worker or the UI thread (ADR 0009).

#![deny(unsafe_code)]

mod cancel;
mod cookie;
mod error;
mod options;
mod proxy;
mod stream;
mod timeline;
mod tls;

pub use cancel::CancelToken;
pub use cookie::{Cookie, CookieJar, CookieJarHandle, Origin, applies};
pub use error::{HttpDiagnostic, HttpError};
pub use options::{ExecuteOptions, HttpVersion, RedirectPolicy, Timeout, options_with_ca};
pub use proxy::{ProxyConfig, ProxyMode};
pub use stream::{StreamEvent, StreamHandle, StreamingEngine};
pub use timeline::{Redactor, SensitiveValue, TimelineSink, VecTimelineSink};
pub use tls::{CaSource, ClientCertificate, TlsConfig};

use heric_model::{Execution, PreparedRequest};

/// Executes one request to completion.
///
/// # Cancellation
///
/// A request carrying a cancelled [`CancelToken`] returns
/// [`HttpError::Cancelled`], which the UI shows as a cancelled send rather than
/// a failure. Cancellation happens through libcurl's progress callback, so it
/// takes effect during the transfer, not between transfers.
pub trait HttpEngine: Send + Sync {
    /// Executes a request.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] only when no response exists. A response with a
    /// 4xx or 5xx status is a successful execution, and a transport failure
    /// that still produced partial output is carried in
    /// [`heric_model::Execution::error`], because the original keeps a failed
    /// response in history with its partial timeline.
    fn execute(
        &self,
        request: &PreparedRequest,
        options: &ExecuteOptions,
    ) -> Result<Execution, HttpError>;

    /// Applies this engine's cookie jar to a request and stores what comes
    /// back.
    ///
    /// Separate from [`HttpEngine::execute`] so a caller without a jar (the CLI
    /// running one request, a test) does not have to construct one.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] when the jar cannot be read or written.
    fn execute_with_cookies(
        &self,
        request: &PreparedRequest,
        options: &ExecuteOptions,
        jar: &dyn CookieJar,
    ) -> Result<Execution, HttpError> {
        let (_request, _options, _jar) = (request, options, jar);
        Err(HttpError::Unsupported)
    }
}

/// Returns the version of `heric-http`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Builds the libcurl-backed engine.
///
/// # Errors
///
/// Returns [`HttpError::Engine`] when libcurl cannot be initialised, which on a
/// statically linked build means the binary is broken rather than the host.
pub fn engine() -> Result<Box<dyn HttpEngine>, HttpError> {
    todo!("M1: construct the curl-backed engine")
}

/// Builds the libcurl-backed streaming engine, used by SSE and chunked
/// responses.
///
/// # Errors
///
/// Returns [`HttpError::Engine`] when libcurl cannot be initialised.
pub fn streaming_engine() -> Result<Box<dyn StreamingEngine>, HttpError> {
    todo!("M1: construct the curl-backed streaming engine")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
