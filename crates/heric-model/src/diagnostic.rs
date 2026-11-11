//! Localisable messages carried by error types.
//!
//! A `thiserror` `Display` string is fixed English and cannot be translated at
//! display time, so every error enum in the workspace also reports a stable
//! message key and its arguments. The GUI resolves the key through Fluent;
//! logs and the CLI use `Display`. See ADR 0012.

/// A stable identifier for a translatable message.
///
/// Keys are dotted and include the crate: `heric-storage.migration.no_source`.
/// The value is deliberately a plain string rather than an enum, so a crate can
/// add a key without changing this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageKey(&'static str);

impl MessageKey {
    /// Creates a key. Keys are compile-time constants by construction.
    #[must_use]
    pub const fn new(key: &'static str) -> Self {
        Self(key)
    }

    /// Returns the key text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for MessageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// A value interpolated into a translatable message.
///
/// Deliberately small: a translation argument is something a translator can
/// reorder inside a sentence, not an arbitrary object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageValue {
    /// Free text, such as a file path or an entity name.
    Text(String),
    /// A whole number, such as a byte count or an HTTP status.
    Integer(i64),
}

impl From<String> for MessageValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for MessageValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<i64> for MessageValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

/// An error that can describe itself in a language other than English.
///
/// Implemented by every crate's error enum. `Display` stays English for logs
/// and the CLI; the GUI asks for the key and the arguments instead.
pub trait Diagnostic {
    /// The message key for this error.
    fn message_key(&self) -> MessageKey;

    /// The arguments the message takes, by translator-facing name.
    fn message_args(&self) -> Vec<(String, MessageValue)>;

    /// An optional key for a suggested next action, shown under the message.
    fn hint_key(&self) -> Option<MessageKey> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_compare_by_text() {
        assert_eq!(MessageKey::new("a.b"), MessageKey::new("a.b"));
        assert_ne!(MessageKey::new("a.b"), MessageKey::new("a.c"));
    }

    #[test]
    fn values_convert_from_the_common_shapes() {
        assert_eq!(MessageValue::from("x"), MessageValue::Text("x".to_owned()));
        assert_eq!(MessageValue::from(7_i64), MessageValue::Integer(7));
    }
}
