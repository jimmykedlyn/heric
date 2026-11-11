//! The UI's end of the core's event channel (ADR 0009).
//!
//! The window never awaits the core and never blocks. Once per frame it asks
//! the channel what has arrived since the last one. The core, for its part,
//! asks for the repaint itself and does so *after* enqueueing, which is why the
//! sending half is bundled with the repaint signal in `EventSink`: a frame can
//! never be drawn stale with no redraw scheduled.

use heric_core::{CoreEvent, EventReceiver, EventSink, RepaintSignal};

/// Creates the channel between the core and the UI.
///
/// The sending half carries the repaint signal, so every event the core emits
/// schedules exactly one redraw after it is visible in the channel. Tests use
/// the same constructor, which is what makes the ordering guaranteed by
/// `EventSink` verifiable from the application side.
#[must_use]
pub fn channel(repaint: RepaintSignal) -> (EventSink, EventBridge) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (EventSink::new(sender, repaint), EventBridge::new(receiver))
}

/// What the UI drains once per frame.
pub struct EventBridge {
    receiver: EventReceiver,
    pending: Vec<CoreEvent>,
}

impl EventBridge {
    /// Wraps the receiving half.
    #[must_use]
    pub fn new(receiver: EventReceiver) -> Self {
        Self {
            receiver,
            pending: Vec::new(),
        }
    }

    /// Collects everything that arrived since the previous call.
    ///
    /// The previous frame's events are released first, so a long session does
    /// not accumulate bodies it already rendered. Draining is non-blocking by
    /// construction: `try_recv` returns immediately when the channel is empty.
    pub fn drain(&mut self) -> &[CoreEvent] {
        self.pending.clear();
        while let Ok(event) = self.receiver.try_recv() {
            self.pending.push(event);
        }
        &self.pending
    }

    /// Whether the core is still sending.
    ///
    /// False once the runtime and its sink are gone, which is what the status
    /// line reports instead of waiting for an event that will never come.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.receiver.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use heric_core::no_repaint;

    fn counting_repaint() -> (RepaintSignal, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&count);
        let signal: RepaintSignal = Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        (signal, count)
    }

    #[test]
    fn each_event_is_drained_exactly_once() {
        let (sink, mut bridge) = channel(counting_repaint().0);
        sink.send(CoreEvent::Failed {
            message: "first".to_owned(),
            fatal: false,
        });
        sink.send(CoreEvent::Failed {
            message: "second".to_owned(),
            fatal: false,
        });

        assert_eq!(bridge.drain().len(), 2);
        assert!(bridge.drain().is_empty());
    }

    #[test]
    fn a_shutdown_ends_the_stream_without_a_repaint() {
        let (sink, mut bridge) = channel(counting_repaint().0);
        sink.send(CoreEvent::Shutdown);
        assert_eq!(bridge.drain().len(), 1);
    }

    #[test]
    fn the_repaint_is_requested_once_per_event_that_wants_one() {
        let (signal, count) = counting_repaint();
        let (sink, mut bridge) = channel(signal);

        sink.send(CoreEvent::Failed {
            message: "one".to_owned(),
            fatal: false,
        });
        assert_eq!(count.load(Ordering::SeqCst), 1);
        sink.send(CoreEvent::Shutdown);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        let _ = bridge.drain();
    }

    #[test]
    fn the_bridge_reports_a_closed_core() {
        let (sink, mut bridge) = channel(no_repaint());
        assert!(bridge.is_open());
        drop(sink);
        assert!(!bridge.is_open());
        assert!(bridge.drain().is_empty());
    }
}
