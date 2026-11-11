//! Proxy resolution.

/// How a request's proxy is decided.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProxyMode {
    /// Follow the operating system's proxy settings.
    ///
    /// The original asks the system for a proxy script and, when a PAC file
    /// returns several results, uses the first. An unparseable value falls back
    /// to a direct connection rather than failing the request, and Heric keeps
    /// that behaviour.
    #[default]
    System,
    /// No proxy.
    Direct,
    /// The configured proxy, ignoring the system.
    Manual,
}

/// A proxy configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    /// How to decide.
    pub mode: ProxyMode,
    /// The proxy URL, used when `mode` is [`ProxyMode::Manual`].
    pub url: Option<String>,
    /// Hosts that bypass the proxy.
    pub no_proxy: Vec<String>,
    /// Username for proxy authentication.
    pub username: Option<String>,
    /// Password for proxy authentication.
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Uses no proxy.
    #[must_use]
    pub fn direct() -> Self {
        Self {
            mode: ProxyMode::Direct,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_is_followed_by_default() {
        assert_eq!(ProxyConfig::default().mode, ProxyMode::System);
        assert_eq!(ProxyConfig::direct().mode, ProxyMode::Direct);
    }
}
