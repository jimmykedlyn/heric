//! HTTP engine for Heric.
//!
//! Executes requests with a timeline faithful to the original client:
//! redirects, auth hooks (delegated to `heric-auth`), cookies, client
//! certificates, proxy support and timeouts. See `docs/inventory/http.md`
//! once the inventory lands.

/// Outcome of a request execution.
#[derive(Debug)]
pub struct ExecutionStub {
    /// Human readable summary, replaced by a real response type in M1.
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_round_trips() {
        let exec = ExecutionStub {
            summary: "GET example.com".to_string(),
        };
        assert_eq!(exec.summary, "GET example.com");
    }
}
