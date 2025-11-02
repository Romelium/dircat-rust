// src/progress.rs

//! Defines a trait for reporting progress of long-running operations.
use indicatif::{ProgressBar, ProgressStyle};

/// A trait for reporting progress, abstracting over specific implementations like `indicatif`.
pub trait ProgressReporter: Send + Sync {
    /// Sets the total number of items to process.
    fn set_length(&self, len: u64);
    /// Sets the current position in the process.
    fn set_position(&self, pos: u64);
    /// Sets a descriptive message for the current operation.
    fn set_message(&self, msg: String);
    /// Finishes the progress reporting.
    fn finish(&self);
    /// Finishes the progress reporting with a final message.
    fn finish_with_message(&self, msg: String);
}

/// A no-op implementation of `ProgressReporter` for non-interactive contexts.
pub struct NoOpProgress;

impl ProgressReporter for NoOpProgress {
    fn set_length(&self, _len: u64) {}
    fn set_position(&self, _pos: u64) {}
    fn set_message(&self, _msg: String) {}
    fn finish(&self) {}
    fn finish_with_message(&self, _msg: String) {}
}

/// An implementation of `ProgressReporter` using the `indicatif` crate.
#[derive(Clone)]
pub struct IndicatifProgress {
    bar: ProgressBar,
}

impl IndicatifProgress {
    /// Creates a new progress bar with a default style.
    pub fn new() -> Self {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Self { bar: pb }
    }
}

impl Default for IndicatifProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter for IndicatifProgress {
    fn set_length(&self, len: u64) {
        self.bar.set_length(len);
    }

    fn set_position(&self, pos: u64) {
        self.bar.set_position(pos);
    }

    fn set_message(&self, msg: String) {
        self.bar.set_message(msg);
    }

    fn finish(&self) {
        self.bar.finish();
    }

    fn finish_with_message(&self, msg: String) {
        self.bar.finish_with_message(msg);
    }
}
