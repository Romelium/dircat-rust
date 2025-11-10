// src/signal.rs

//! Provides signal handling for graceful shutdown.

use crate::cancellation::CancellationToken;
use anyhow::{Context, Result};

/// Sets up a handler for Ctrl+C (SIGINT).
///
/// This function registers a handler that listens for the interrupt signal.
/// When the signal is caught, it flips the value of the returned `AtomicBool`
/// from `true` to `false`. Long-running operations in the application can
/// periodically check this boolean to gracefully terminate.
///
/// # Returns
/// A `CancellationToken` that will be cancelled when the signal is caught.
///
/// # Errors
/// Returns an error if the signal handler cannot be set.
///
/// # Examples
///
/// ```no_run
/// use dircat::signal::setup_signal_handler;
/// use std::thread;
/// use std::time::Duration;
///
/// # fn main() -> anyhow::Result<()> {
/// // In a real app, you would call this once at the start.
/// let token = setup_signal_handler()?;
///
/// // Simulate a long-running task
/// while !token.is_cancelled() {
///     println!("Working...");
///     thread::sleep(Duration::from_secs(1));
/// }
///
/// println!("Work cancelled, shutting down.");
/// # Ok(())
/// # }
/// ```
pub fn setup_signal_handler() -> Result<CancellationToken> {
    let token = CancellationToken::new();
    let token_clone = token.clone();

    ctrlc::set_handler(move || {
        log::info!("Ctrl+C signal received, attempting graceful shutdown.");
        token_clone.cancel();
        // You could add a second Ctrl+C handler here for immediate exit if needed.
    })
    .context("Failed to set Ctrl+C signal handler")?;

    Ok(token)
}

// Note: Testing signal handlers directly is complex and often skipped
// or handled via integration tests that send signals to the process.
