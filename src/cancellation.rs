//! Provides a token-based mechanism for graceful cancellation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A token that can be used to signal cancellation to long-running operations.
///
/// This struct is a cloneable, thread-safe wrapper around an `Arc<AtomicBool>`.
/// It provides a clear and ergonomic API for checking if an operation should be
/// gracefully terminated.
///
/// # Examples
///
/// ```
/// use dircat::CancellationToken;
/// use std::thread;
/// use std::time::Duration;
///
/// let token = CancellationToken::new();
/// let token_clone = token.clone();
///
/// let handle = thread::spawn(move || {
///     while !token_clone.is_cancelled() {
///         println!("Working...");
///         thread::sleep(Duration::from_millis(100));
///     }
///     println!("Work cancelled.");
/// });
///
/// // Let the thread work for a bit
/// thread::sleep(Duration::from_millis(250));
///
/// // Signal cancellation from another thread
/// token.cancel();
///
/// handle.join().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new `CancellationToken` in a non-cancelled state.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)), // false means not cancelled
        }
    }

    /// Signals cancellation.
    ///
    /// This sets the token's state to "cancelled". All subsequent calls to
    /// `is_cancelled()` on this token or any of its clones will return `true`.
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    /// Checks if the token has been cancelled.
    ///
    /// *   Returns `true` if `cancel()` has been called on this token or any of its clones.
    /// *   Returns `false` otherwise.
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::Relaxed)
    }
}

/// Creates a new `CancellationToken` in a non-cancelled state.
///
/// This is equivalent to calling `CancellationToken::new()`.
///
/// # Examples
///
/// ```
/// use dircat::CancellationToken;
///
/// let token: CancellationToken = Default::default();
/// assert!(!token.is_cancelled());
/// ```
impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
