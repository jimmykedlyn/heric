//! The timeline sink and redaction.

use heric_model::TimelineEntry;

/// Receives timeline entries as they happen.
///
/// Redaction is a property of the sink, not of the emitter: the engine pushes
/// everything, and whichever sink writes to disk drops or masks registered
/// values first. That way the on-disk timeline is safe and the in-memory one is
/// complete for the console.
pub trait TimelineSink: Send + Sync {
    /// Records one entry.
    fn push(&self, entry: &TimelineEntry);

    /// Whether the sink wants data-in entries.
    ///
    /// A response body can be megabytes, so a sink that only shows headers must
    /// be able to say so before the bytes are stringified.
    fn wants_body(&self) -> bool {
        false
    }
}

/// A timeline sink that keeps everything in memory.
///
/// Used by the engine's tests and by the fake engine; a real GUI shows entries
/// as they arrive instead of collecting them.
#[derive(Debug, Default)]
pub struct VecTimelineSink {
    entries: std::sync::Mutex<Vec<TimelineEntry>>,
}

impl VecTimelineSink {
    /// An empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything recorded so far.
    ///
    /// A poisoned lock yields the entries recorded before the panic rather than
    /// propagating: a sink is diagnostic, and losing diagnostics to a panic
    /// elsewhere would be worse than returning a short list.
    #[must_use]
    pub fn entries(&self) -> Vec<TimelineEntry> {
        self.entries
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl TimelineSink for VecTimelineSink {
    fn push(&self, entry: &TimelineEntry) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(entry.clone());
        }
    }

    fn wants_body(&self) -> bool {
        true
    }
}

/// A value that must not appear in a persisted timeline.
///
/// The original collects these in a registry as it renders, then masks them on
/// the way out. Registration happens during rendering, which is why the sink and
/// the renderer share one registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitiveValue {
    /// The secret itself.
    pub value: String,
    /// What it is, for the report.
    pub label: String,
}

/// Masks registered values in text destined for disk.
#[derive(Debug, Default)]
pub struct Redactor {
    values: Vec<SensitiveValue>,
    mask: String,
}

impl Redactor {
    /// Creates a redactor with the given replacement text.
    #[must_use]
    pub fn new(mask: impl Into<String>) -> Self {
        Self {
            values: Vec::new(),
            mask: mask.into(),
        }
    }

    /// Registers a value that must never be written.
    ///
    /// Empty values are ignored: masking an empty string would put the mask
    /// between every character.
    pub fn register(&mut self, value: SensitiveValue) {
        if !value.value.is_empty() {
            self.values.push(value);
        }
    }

    /// Returns the text with every registered value replaced.
    #[must_use]
    pub fn redact<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        let mut out = std::borrow::Cow::Borrowed(text);
        for value in &self.values {
            if out.contains(&value.value) {
                out = std::borrow::Cow::Owned(out.replace(&value.value, &self.mask));
            }
        }
        out
    }

    /// How many values are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_replaces_every_occurrence() {
        let mut redactor = Redactor::new("***");
        redactor.register(SensitiveValue {
            value: "s3cret".to_owned(),
            label: "token".to_owned(),
        });
        assert_eq!(
            redactor
                .redact("Authorization: Bearer s3cret (s3cret)")
                .as_ref(),
            "Authorization: Bearer *** (***)"
        );
    }

    #[test]
    fn an_empty_secret_is_not_registered() {
        let mut redactor = Redactor::new("***");
        redactor.register(SensitiveValue {
            value: String::new(),
            label: "token".to_owned(),
        });
        assert!(redactor.is_empty());
        assert_eq!(redactor.redact("abc").as_ref(), "abc");
    }

    #[test]
    fn clean_text_is_borrowed_not_copied() {
        let redactor = Redactor::new("***");
        assert!(matches!(
            redactor.redact("nothing secret"),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn the_vec_sink_keeps_entries_in_order() {
        use heric_model::TimelineKind;
        use std::time::Duration;

        let sink = VecTimelineSink::new();
        for (i, kind) in [TimelineKind::HeaderOut, TimelineKind::HeaderIn]
            .into_iter()
            .enumerate()
        {
            sink.push(&TimelineEntry {
                at: Duration::from_millis(i as u64),
                kind,
                text: format!("entry {i}"),
            });
        }
        let entries = sink.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].kind, TimelineKind::HeaderOut);
        assert!(sink.wants_body());
    }
}
