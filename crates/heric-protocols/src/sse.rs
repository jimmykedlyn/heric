//! Server-sent events.

use std::time::Duration;

/// One server-sent event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseEvent {
    /// The `id:` field, when present.
    pub id: Option<String>,
    /// The `event:` field, when present.
    pub event: Option<String>,
    /// The concatenated `data:` fields.
    pub data: String,
    /// The `retry:` value, in milliseconds.
    pub retry: Option<u64>,
    /// How long the event took to arrive, for the request's timeline.
    pub at: Duration,
}

impl SseEvent {
    /// Whether the event carries anything worth storing.
    ///
    /// A keep-alive comment produces an empty event, which the original does not
    /// persist: it exists only to hold the connection open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.event.is_none() && self.id.is_none()
    }
}

/// How a stream reconnects.
///
/// The original offers a repeat-on-interval option for event streams, which is
/// cleared when the pane unmounts or the user disconnects. It is not the same as
/// the server's `retry:` hint: this one is a user choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SseReconnect {
    /// Do not reconnect.
    #[default]
    Never,
    /// Reconnect after a fixed interval.
    Every(Duration),
}

impl SseReconnect {
    /// The delay before the next attempt, honouring the server's `retry:` field
    /// when the user asked for a fixed interval.
    #[must_use]
    pub fn delay(self, server_hint: Option<u64>) -> Option<Duration> {
        match self {
            Self::Never => None,
            Self::Every(interval) => Some(match server_hint {
                Some(millis) => Duration::from_millis(millis),
                None => interval,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keepalive_is_empty() {
        assert!(SseEvent::default().is_empty());
    }

    #[test]
    fn data_alone_is_not_empty() {
        let event = SseEvent {
            data: "hello".to_owned(),
            ..SseEvent::default()
        };
        assert!(!event.is_empty());
    }

    #[test]
    fn never_reconnecting_returns_no_delay() {
        assert_eq!(SseReconnect::Never.delay(Some(1_000)), None);
    }

    #[test]
    fn a_server_hint_wins_over_the_interval() {
        assert_eq!(
            SseReconnect::Every(Duration::from_secs(5)).delay(Some(1_000)),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            SseReconnect::Every(Duration::from_secs(5)).delay(None),
            Some(Duration::from_secs(5))
        );
    }
}
