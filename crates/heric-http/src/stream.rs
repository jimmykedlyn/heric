//! Streaming: SSE, chunked responses and protocol upgrades.

use heric_model::PreparedRequest;

use crate::{ExecuteOptions, HttpError};

/// One thing that happened on a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// The connection is established and the first bytes arrived.
    Open {
        /// Status code.
        status: u16,
    },
    /// A chunk of the body arrived.
    Data(Vec<u8>),
    /// The stream ended normally.
    End,
    /// The stream failed.
    Error(String),
}

/// An open stream.
///
/// The handle owns the transfer and must be closed explicitly, because a
/// dropped handle would leave libcurl's thread blocked on a socket that nothing
/// is reading.
pub struct StreamHandle {
    receiver: std::sync::mpsc::Receiver<StreamEvent>,
    closer: Box<dyn FnOnce() + Send>,
}

impl StreamHandle {
    /// Wraps a receiver and a closer, for implementations and tests.
    #[must_use]
    pub fn new(
        receiver: std::sync::mpsc::Receiver<StreamEvent>,
        closer: Box<dyn FnOnce() + Send>,
    ) -> Self {
        Self { receiver, closer }
    }

    /// Waits for the next event.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::StreamClosed`] when the producer is gone.
    pub fn next_event(&self) -> Result<StreamEvent, HttpError> {
        self.receiver.recv().map_err(|_| HttpError::StreamClosed)
    }

    /// Rows arriving from the stream.
    #[must_use]
    pub fn events(&self) -> std::sync::mpsc::Iter<'_, StreamEvent> {
        self.receiver.iter()
    }

    /// Closes the stream, releasing the engine's resources.
    pub fn close(self) {
        (self.closer)();
    }
}

impl std::fmt::Debug for StreamHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamHandle").finish_non_exhaustive()
    }
}

/// Opens a stream instead of buffering a response.
///
/// Separate from [`crate::HttpEngine`] because most callers want the buffered
/// path and should not have to handle a handle.
pub trait StreamingEngine: Send + Sync {
    /// Opens a stream.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] when the connection cannot be established.
    fn open(
        &self,
        request: &PreparedRequest,
        options: &ExecuteOptions,
    ) -> Result<StreamHandle, HttpError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_yields_events_and_closes_once() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&closed);
        let handle = StreamHandle::new(
            rx,
            Box::new(move || flag.store(true, std::sync::atomic::Ordering::SeqCst)),
        );

        tx.send(StreamEvent::Open { status: 200 })?;
        assert_eq!(handle.next_event()?, StreamEvent::Open { status: 200 });
        drop(tx);
        assert!(matches!(handle.next_event(), Err(HttpError::StreamClosed)));

        handle.close();
        assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
        Ok(())
    }
}
