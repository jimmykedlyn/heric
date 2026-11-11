//! The event stream from the runtime to the UI.

use heric_model::{Id, TimelineEntry};

/// Something that happened in the background and the UI should see.
///
/// Unbounded on purpose. A bounded channel would either block the runtime (which
/// serialises every send behind the slowest frame) or drop events (which loses a
/// response the user asked for). The volume is bounded in practice by how fast a
/// user can press send, and the largest payload, a response body, is a handle
/// into the blob store rather than the bytes themselves.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// A document changed, from any source.
    Changed {
        /// What changed.
        change: heric_model::Change,
    },

    /// A request started.
    SendStarted {
        /// The request.
        request: Id,
        /// Which run, so a stale event from a superseded send is ignored.
        run: u64,
    },

    /// A timeline entry arrived mid-flight.
    Timeline {
        /// The request.
        request: Id,
        /// Which run.
        run: u64,
        /// The entry, already redacted by the sink.
        entry: TimelineEntry,
    },

    /// A request finished.
    SendFinished {
        /// The request.
        request: Id,
        /// Which run.
        run: u64,
        /// The outcome.
        outcome: Box<crate::SendOutcome>,
    },

    /// A send was cancelled, either by the user or by a closing window.
    SendCancelled {
        /// The request.
        request: Id,
        /// Which run.
        run: u64,
    },

    /// A realtime session changed state.
    SessionChanged {
        /// Which session.
        session: crate::OpenSession,
        /// A short description for the log line.
        detail: String,
    },

    /// A session produced a message.
    SessionMessage {
        /// Which session.
        session: crate::OpenSession,
        /// The message, already redacted.
        text: String,
    },

    /// Import or export finished.
    ImportFinished {
        /// How many documents were created, or written on export.
        count: usize,
        /// Whether the report needs the user's attention.
        needs_action: bool,
    },

    /// A background task failed.
    Failed {
        /// What the user should read.
        message: String,
        /// Whether the failure is fatal to the session.
        fatal: bool,
    },

    /// The core is shutting down; the UI should stop expecting events.
    Shutdown,
}

impl CoreEvent {
    /// Whether this event should force a repaint.
    ///
    /// Every event does, which is why the signal is invoked after enqueueing
    /// rather than being the caller's responsibility: forgetting it once leaves a
    /// frame stale with no scheduled redraw.
    #[must_use]
    pub fn wants_repaint(&self) -> bool {
        !matches!(self, Self::Shutdown)
    }

    /// Whether the event carries a response body that the caller must drop
    /// promptly, so a large body is not held in memory by a queued event.
    #[must_use]
    pub fn is_heavy(&self) -> bool {
        matches!(self, Self::SendFinished { outcome, .. } if matches!(&outcome.execution, Some(execution) if execution.bytes_read > 1_048_576))
    }
}

/// The receiving half of the event stream.
pub type EventReceiver = tokio::sync::mpsc::UnboundedReceiver<CoreEvent>;

/// Asks the UI to redraw.
///
/// A closure rather than an `egui::Context` so the core does not depend on a GUI
/// toolkit: the CLI passes a no-op, and the test harness passes a counter.
pub type RepaintSignal = std::sync::Arc<dyn Fn() + Send + Sync>;

/// A no-op repaint signal, for the CLI and for tests.
#[must_use]
pub fn no_repaint() -> RepaintSignal {
    std::sync::Arc::new(|| {})
}

/// The sending half the core uses, with the repaint signal attached.
///
/// Bundling the two is the mechanism that makes "repaint after enqueue" the only
/// available path.
#[derive(Clone)]
pub struct EventSink {
    sender: tokio::sync::mpsc::UnboundedSender<CoreEvent>,
    repaint: RepaintSignal,
}

impl EventSink {
    /// Builds a sink.
    #[must_use]
    pub fn new(
        sender: tokio::sync::mpsc::UnboundedSender<CoreEvent>,
        repaint: RepaintSignal,
    ) -> Self {
        Self { sender, repaint }
    }

    /// Sends an event, then asks for a repaint.
    ///
    /// Ignoring a send failure is correct: the only way it fails is that the
    /// receiver was dropped, which means the UI is shutting down and there is
    /// nobody left to repaint.
    pub fn send(&self, event: CoreEvent) {
        let wants_repaint = event.wants_repaint();
        if self.sender.send(event).is_ok() && wants_repaint {
            (self.repaint)();
        }
    }

    /// Whether the receiver is gone.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

impl std::fmt::Debug for EventSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSink")
            .field("closed", &self.is_closed())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heric_model::Execution;

    #[test]
    fn sending_repaints_after_the_event_is_queued() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let repainted = std::sync::Arc::new(AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&repainted);
        let sink = EventSink::new(
            sender,
            std::sync::Arc::new(move || flag.store(true, Ordering::SeqCst)),
        );

        sink.send(CoreEvent::Failed {
            message: "boom".to_owned(),
            fatal: false,
        });

        // The event must already be in the channel when the repaint fires.
        assert!(repainted.load(Ordering::SeqCst));
        assert!(receiver.try_recv().is_ok());
    }

    #[test]
    fn shutdown_does_not_ask_for_a_repaint() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let repainted = std::sync::Arc::new(AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&repainted);
        let sink = EventSink::new(
            sender,
            std::sync::Arc::new(move || flag.store(true, Ordering::SeqCst)),
        );
        sink.send(CoreEvent::Shutdown);
        assert!(!repainted.load(Ordering::SeqCst));
    }

    #[test]
    fn a_closed_receiver_is_reported() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = EventSink::new(sender, no_repaint());
        assert!(!sink.is_closed());
        drop(receiver);
        assert!(sink.is_closed());
        // Sending into a closed sink must not panic.
        sink.send(CoreEvent::Shutdown);
    }

    #[test]
    fn a_large_response_is_marked_heavy() {
        let mut execution = Execution::ok(200);
        execution.bytes_read = 2_000_000;
        let event = CoreEvent::SendFinished {
            request: Id::new("req_1"),
            run: 1,
            outcome: Box::new(crate::SendOutcome {
                execution: Some(execution),
                ..crate::SendOutcome::default()
            }),
        };
        assert!(event.is_heavy());
    }
}
