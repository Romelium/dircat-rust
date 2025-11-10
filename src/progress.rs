// src/progress.rs

//! Defines a trait for reporting progress of long-running operations.
#[cfg(feature = "progress")]
use indicatif::{ProgressBar, ProgressStyle};

/// A trait for reporting progress, abstracting over specific implementations like `indicatif`.
///
/// # Examples
///
/// ```
/// use dircat::progress::ProgressReporter;
/// use std::sync::Mutex;
///
/// // A mock reporter that just stores the last message.
/// struct MockProgress {
///     last_message: Mutex<String>,
/// }
/// impl ProgressReporter for MockProgress {
///     fn set_length(&self, len: u64) {}
///     fn set_position(&self, pos: u64) {}
///     fn set_message(&self, msg: String) {
///         *self.last_message.lock().unwrap() = msg;
///     }
///     fn finish(&self) {}
///     fn finish_with_message(&self, msg: String) {
///         *self.last_message.lock().unwrap() = msg;
///     }
/// }
///
/// let reporter = MockProgress { last_message: Mutex::new("".to_string()) };
/// reporter.set_message("Working...".to_string());
/// assert_eq!(*reporter.last_message.lock().unwrap(), "Working...");
/// reporter.finish_with_message("Done.".to_string());
/// assert_eq!(*reporter.last_message.lock().unwrap(), "Done.");
/// ```
pub trait ProgressReporter: Send + Sync {
    /// Sets the total number of items to process.
    fn set_length(&self, len: u64);
    /// Sets the current position in the process.
    fn set_position(&self, pos: u64);
    /// Sets a descriptive message for the current operation (e.g., "Cloning...").
    fn set_message(&self, msg: String);
    /// Finishes the progress reporting, hiding the progress bar.
    fn finish(&self);
    /// Finishes the progress reporting with a final message and hides the progress bar.
    fn finish_with_message(&self, msg: String);
}

/// A `ProgressReporter` that does nothing.
///
/// This is used as a default or in non-interactive environments where a progress
/// bar is not desired.
pub struct NoOpProgress;

impl ProgressReporter for NoOpProgress {
    fn set_length(&self, _len: u64) {}
    fn set_position(&self, _pos: u64) {}
    fn set_message(&self, _msg: String) {}
    fn finish(&self) {}
    fn finish_with_message(&self, _msg: String) {}
}

/// An implementation of `ProgressReporter` using the `indicatif` crate.
#[cfg(feature = "progress")]
#[derive(Clone)]
pub struct IndicatifProgress {
    bar: ProgressBar,
}

#[cfg(feature = "progress")]
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

#[cfg(feature = "progress")]
impl Default for IndicatifProgress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "progress")]
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
