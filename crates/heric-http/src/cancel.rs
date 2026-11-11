//! Cancellation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A handle that stops an in-flight execution.
///
/// Backed by an atomic flag rather than by dropping a future: libcurl expects
/// its progress callback to return abort, and a future that is dropped on
/// another thread cannot reach into the C call that is blocking. The token is
/// therefore cloned into the callback and read on every transfer tick.
///
/// # Cloning
///
/// Clones share one flag. Giving one clone to the UI and another to the engine
/// is the intended use.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// A token that has not been cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    ///
    /// Idempotent, and safe to call from any thread, including the UI thread
    /// mid-frame.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    ///
    /// `Acquire` pairs with the `Release` store so a callback on another thread
    /// observes the flag without a lock.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    /// Whether this token shares its flag with another.
    #[must_use]
    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.flag, &other.flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_is_not_cancelled() {
        assert!(!CancelToken::new().is_cancelled());
    }

    #[test]
    fn cancelling_one_clone_cancels_them_all() {
        let token = CancelToken::new();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
        assert!(token.shares_with(&clone));
    }

    #[test]
    fn independent_tokens_do_not_share_state() {
        let a = CancelToken::new();
        let b = CancelToken::new();
        a.cancel();
        assert!(!b.is_cancelled());
        assert!(!a.shares_with(&b));
    }
}
