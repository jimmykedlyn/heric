//! Secrets, and the states a secret can be in.

use std::sync::Arc;

/// Points at a secret without holding it.
///
/// The original stores credential material inside the document database. Heric
/// moves it to the operating system's keychain and leaves this reference in the
/// document, so the database never contains a live password (ADR 0011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    /// Which document owns the secret.
    pub doc: heric_model::Id,
    /// Which field inside that document.
    pub field: String,
    /// Namespaces the entry in the keychain.
    pub service: String,
}

/// The four states a secret can be in.
///
/// The fourth is the one the migration creates: the original encrypted some
/// values with Electron's `safeStorage`, so the ciphertext was copied verbatim
/// and Heric can never decrypt it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretState {
    /// Readable and ready to use.
    Present(String),
    /// Ciphertext copied from the original; the user must type the value once.
    PendingReEntry {
        /// What the ciphertext came from, for the migration report.
        source: String,
    },
    /// Nothing was ever stored.
    Absent,
    /// The backend exists but refused: locked keychain, denied prompt, no
    /// session keyring.
    Unavailable {
        /// Why.
        reason: String,
    },
}

impl SecretState {
    /// The value, when one is usable.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Present(value) => Some(value),
            _ => None,
        }
    }

    /// Whether the user can fix this state by typing something.
    #[must_use]
    pub fn needs_user(&self) -> bool {
        matches!(self, Self::PendingReEntry { .. })
    }

    /// Whether the state means the request cannot be sent.
    #[must_use]
    pub fn blocks_send(&self) -> bool {
        !matches!(self, Self::Present(_))
    }
}

impl From<String> for SecretState {
    fn from(value: String) -> Self {
        Self::Present(value)
    }
}

impl From<&str> for SecretState {
    fn from(value: &str) -> Self {
        Self::Present(value.to_owned())
    }
}

/// Asks the user for a value.
///
/// Only the GUI can answer this; the CLI either supplies the value on the
/// command line or fails with [`SecretState::Unavailable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPrompt {
    /// What is being asked for.
    pub title: String,
    /// Where it will be stored.
    pub secret: SecretRef,
    /// Whether to echo the input.
    pub secret_input: bool,
}

/// Reads and writes credentials.
///
/// Deliberately not the same trait as `heric_template::TagHost::prompt`: a
/// template prompt is mid-render and has a timeout, while a credential prompt is
/// a settings action with no deadline. Merging them would force the renderer to
/// know about keychains.
pub trait SecretResolver: Send + Sync {
    /// Looks up a secret.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError`] when the backend fails hard, as opposed to
    /// reporting a missing or pending secret through [`SecretState`].
    fn get(&self, secret: &SecretRef) -> Result<SecretState, crate::AuthError>;

    /// Stores a secret the user just typed.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError`] when the write fails.
    fn set(&self, secret: &SecretRef, value: &str) -> Result<(), crate::AuthError>;

    /// Forgets a secret.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError`] when the delete fails.
    fn remove(&self, secret: &SecretRef) -> Result<(), crate::AuthError>;

    /// Whether writes will work, so the UI can disable a field instead of
    /// failing when the user submits it.
    fn can_write(&self) -> bool;
}

/// A shared resolver.
pub type SecretResolverHandle = Arc<dyn SecretResolver>;

/// A resolver that keeps secrets in memory.
///
/// The CLI uses it when the user passes `--secret` explicitly, and the tests use
/// it everywhere.
#[derive(Debug, Default)]
pub struct MemorySecrets {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl MemorySecrets {
    /// An empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a value for a reference.
    ///
    /// # Errors
    ///
    /// Returns [`crate::AuthError::NoBackend`] if the lock is poisoned, which
    /// means another thread panicked while holding it.
    pub fn insert(
        &self,
        secret: &SecretRef,
        value: impl Into<String>,
    ) -> Result<(), crate::AuthError> {
        let mut map = self
            .values
            .lock()
            .map_err(|_| crate::AuthError::NoBackend)?;
        map.insert(key(secret), value.into());
        Ok(())
    }
}

fn key(secret: &SecretRef) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        secret.service,
        secret.doc.as_str(),
        secret.field
    )
}

impl SecretResolver for MemorySecrets {
    fn get(&self, secret: &SecretRef) -> Result<SecretState, crate::AuthError> {
        let map = self
            .values
            .lock()
            .map_err(|_| crate::AuthError::NoBackend)?;
        Ok(map
            .get(&key(secret))
            .map(|value| SecretState::Present(value.clone()))
            .unwrap_or(SecretState::Absent))
    }

    fn set(&self, secret: &SecretRef, value: &str) -> Result<(), crate::AuthError> {
        self.insert(secret, value)
    }

    fn remove(&self, secret: &SecretRef) -> Result<(), crate::AuthError> {
        let mut map = self
            .values
            .lock()
            .map_err(|_| crate::AuthError::NoBackend)?;
        map.remove(&key(secret));
        Ok(())
    }

    fn can_write(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> SecretRef {
        SecretRef {
            doc: heric_model::Id::new("req_1"),
            field: "bearer_token".to_owned(),
            service: "heric".to_owned(),
        }
    }

    #[test]
    fn a_missing_secret_is_absent_not_an_error() -> Result<(), crate::AuthError> {
        let secrets = MemorySecrets::new();
        assert_eq!(secrets.get(&reference())?, SecretState::Absent);
        Ok(())
    }

    #[test]
    fn a_stored_secret_round_trips() -> Result<(), crate::AuthError> {
        let secrets = MemorySecrets::new();
        secrets.set(&reference(), "abc")?;
        let state = secrets.get(&reference())?;
        assert_eq!(state.value(), Some("abc"));
        secrets.remove(&reference())?;
        assert!(secrets.get(&reference())?.blocks_send());
        Ok(())
    }

    #[test]
    fn pending_reentry_asks_for_the_user() {
        let state = SecretState::PendingReEntry {
            source: "Bearer token".to_owned(),
        };
        assert!(state.needs_user());
        assert!(state.blocks_send());
        assert_eq!(state.value(), None);
    }
}
