// src/signal.rs

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Sets up a handler for Ctrl+C (SIGINT).
/// Returns an Arc<AtomicBool> that will be set to true when the signal is caught.
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
