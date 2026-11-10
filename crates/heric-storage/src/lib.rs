//! Persistence for Heric.
//!
//! Owns the on-disk storage format (chosen in a Phase 1 ADR) and the
//! reader/migrator for existing Insomnia NeDB data directories on Windows,
//! Linux and macOS. See `docs/inventory/storage.md` once the inventory lands.

/// Returns the default data directory for the current platform.
///
/// Placeholder until the storage format ADR lands; uses the
/// `directories` crate conventions when implemented.
#[must_use]
pub fn default_data_dir_placeholder() -> &'static str {
    "heric"
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_is_non_empty() {
        assert!(!super::default_data_dir_placeholder().is_empty());
    }
}
