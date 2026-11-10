//! Code snippet generation for Heric.
//!
//! Replacement for httpsnippet: turns an HTTP request into snippets for
//! cURL, fetch, and many client libraries. Golden tests come from the
//! original test tables. See `docs/inventory/io-and-cli.md` once the
//! inventory lands.

/// Targets a snippet can be generated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// cURL command line.
    Curl,
    /// JavaScript fetch API.
    Fetch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_distinguishable() {
        assert_ne!(Target::Curl, Target::Fetch);
    }
}
