//! Application core for Heric.
//!
//! Orchestrates the other crates into application services (create/send/
//! edit requests, manage environments) and exposes an event stream for the
//! GUI and CLI. The GUI never blocks: it talks to this core over channels
//! backed by a background tokio runtime.

/// Reports the core library version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        let v = super::version();
        assert!(v.split('.').count() == 3, "not semver: {v}");
    }
}
