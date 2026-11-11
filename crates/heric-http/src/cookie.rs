//! The cookie jar.

use heric_model::Id;

/// Where a cookie came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user typed it.
    ///
    /// The original allows template syntax in a manual cookie and does not
    /// re-render a response cookie (`HTTP-023`).
    Manual,
    /// The server set it.
    Response,
}

/// One cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Stable identifier, so an edit updates rather than duplicates.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Value.
    pub value: String,
    /// Domain it applies to.
    pub domain: String,
    /// Path it applies to.
    pub path: String,
    /// Whether it is limited to this host.
    pub host_only: bool,
    /// Whether it is only sent over TLS.
    pub secure: bool,
    /// Whether it is hidden from scripts.
    pub http_only: bool,
    /// Expiry in Unix seconds, or `None` for a session cookie.
    pub expires: Option<i64>,
    /// Where it came from.
    pub origin: Origin,
}

/// A per-workspace cookie jar.
///
/// Persisted in the store, serialised to the Netscape format libcurl reads.
/// Synchronous like every other port.
pub trait CookieJar: Send + Sync {
    /// Cookies to send to a URL.
    ///
    /// # Errors
    ///
    /// Returns [`crate::HttpError`] when the jar cannot be read.
    fn cookies_for(&self, url: &str) -> Result<Vec<Cookie>, crate::HttpError>;

    /// Stores cookies set by a response.
    ///
    /// # Errors
    ///
    /// Returns [`crate::HttpError`] when the write fails.
    fn store(&self, url: &str, cookies: &[Cookie]) -> Result<(), crate::HttpError>;

    /// Removes every cookie.
    ///
    /// # Errors
    ///
    /// Returns [`crate::HttpError`] when the write fails.
    fn clear(&self) -> Result<(), crate::HttpError>;
}

/// A shared handle to a jar.
pub type CookieJarHandle = std::sync::Arc<dyn CookieJar>;

/// Whether a cookie applies to a URL, following the original's matching rules.
///
/// Kept here as a free function so the memory and SQLite jars agree on
/// semantics instead of each implementing its own.
#[must_use]
pub fn applies(cookie: &Cookie, host: &str, path: &str, secure: bool) -> bool {
    if cookie.secure && !secure {
        return false;
    }
    let domain_matches = if cookie.host_only {
        host.eq_ignore_ascii_case(&cookie.domain)
    } else {
        host.eq_ignore_ascii_case(&cookie.domain)
            || host
                .to_ascii_lowercase()
                .ends_with(&format!(".{}", cookie.domain.to_ascii_lowercase()))
    };
    domain_matches && path.starts_with(&cookie.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(domain: &str, host_only: bool) -> Cookie {
        Cookie {
            id: Id::new("c1"),
            name: "session".to_owned(),
            value: "x".to_owned(),
            domain: domain.to_owned(),
            path: "/".to_owned(),
            host_only,
            secure: false,
            http_only: false,
            expires: None,
            origin: Origin::Response,
        }
    }

    #[test]
    fn a_host_only_cookie_does_not_leak_to_subdomains() {
        assert!(applies(
            &cookie("example.com", true),
            "example.com",
            "/",
            false
        ));
        assert!(!applies(
            &cookie("example.com", true),
            "api.example.com",
            "/",
            false
        ));
    }

    #[test]
    fn a_domain_cookie_reaches_subdomains() {
        assert!(applies(
            &cookie("example.com", false),
            "api.example.com",
            "/",
            false
        ));
    }

    #[test]
    fn a_secure_cookie_needs_tls() {
        let mut c = cookie("example.com", false);
        c.secure = true;
        assert!(!applies(&c, "example.com", "/", false));
        assert!(applies(&c, "example.com", "/", true));
    }
}
