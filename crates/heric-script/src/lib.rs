//! Sandboxed scripting for Heric.
//!
//! Runs pre-request and after-response scripts against the `insomnia.*`
//! API surface, using the JS engine chosen in the Phase 1 ADR
//! (rquickjs vs boa vs deno_core). See `docs/inventory/scripting.md` once
//! the inventory lands.

/// Error type for script execution.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    /// The script exceeded its time budget.
    #[error("script timed out after {0:?}")]
    Timeout(std::time::Duration),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_message_includes_duration() {
        let err = ScriptError::Timeout(std::time::Duration::from_millis(1500));
        assert_eq!(err.to_string(), "script timed out after 1.5s");
    }
}
