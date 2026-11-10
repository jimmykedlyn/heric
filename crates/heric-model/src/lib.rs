//! Domain types for Heric.
//!
//! This crate holds the serializable data model shared by storage, the HTTP
//! engine and the UI: workspaces, requests, environments and related IDs.
//! It performs no I/O; persistence lives in `heric-storage`.
//!
//! Layout is derived from the Insomnia data model (see
//! `docs/inventory/models.md` once the Phase 0.3 inventory lands).

/// Strongly typed identifier for domain entities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Id(String);

impl Id {
    /// Creates an ID from a raw string without validating its format.
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Returns the raw string form of this ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Generates a fresh random identifier.
#[must_use]
pub fn generate_id() -> Id {
    Id(uuid::Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_serializes_transparently() -> Result<(), Box<dyn std::error::Error>> {
        let id = Id::new("wrk_123");
        let json = serde_json::to_string(&id)?;
        assert_eq!(json, r#""wrk_123""#);
        let back: Id = serde_json::from_str(&json)?;
        assert_eq!(back, id);
        Ok(())
    }

    #[test]
    fn generate_id_is_unique() {
        let a = generate_id();
        let b = generate_id();
        assert_ne!(a, b);
        assert!(!a.as_str().is_empty());
    }
}
