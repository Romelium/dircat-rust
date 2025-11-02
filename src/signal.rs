// src/signal.rs

//! Provides signal handling for graceful shutdown.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Sets up a handler for Ctrl+C (SIGINT).
///
/// This function registers a handler that listens for the interrupt signal.
/// When the signal is caught, it flips the value of the returned `AtomicBool`
/// from `true` to `false`. Long-running operations in the application can
/// periodically check this boolean to gracefully terminate.
///
/// # Returns
/// An `Arc<AtomicBool>` that will be set to `false` when the signal is caught.
///
/// # Errors
/// Returns an error if the signal handler cannot be set.
pub fn setup_signal_handler() -> Result<Arc<AtomicBool>> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        log::info!("Ctrl+C signal received, attempting graceful shutdown.");
        r.store(false, Ordering::SeqCst);
        // You could add a second Ctrl+C handler here for immediate exit if needed.
    })
    .context("Failed to set Ctrl+C signal handler")?;

    Ok(running)
}

// Note: Testing signal handlers directly is complex and often skipped
// or handled via integration tests that send signals to the process.
