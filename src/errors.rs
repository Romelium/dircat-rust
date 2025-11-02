//! Defines application-specific error types.
//!
//! This module provides the `AppError` enum, which categorizes common errors
//! that can occur during execution, offering more context than generic I/O or
//! `anyhow` errors.

use thiserror::Error;

/// Application-specific errors used throughout `dircat`.
///
/// This enum defines categorized errors that can occur during the execution,
/// providing more context than generic `std::io::Error` or `anyhow::Error`.
#[derive(Error, Debug)]
pub enum AppError {
    // --- I/O Errors ---
    /// Error occurring during file or directory access (read, write, metadata).
    #[error("I/O error accessing path '{path}': {source}")]
    IoError {
        /// The path that caused the I/O error.
        path: String, // Use String to avoid lifetime issues if PathBuf is dropped
        /// The underlying `std::io::Error`.
        #[source]
        source: std::io::Error,
    },

    // --- Configuration Errors ---
    /// Generic error related to invalid configuration settings or combinations.
    /// Often used when validation fails after initial parsing.
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    // --- Clipboard Errors ---
    /// Error related to clipboard operations (copying).
    #[error("Clipboard error: {0}")]
    ClipboardError(String), // arboard::Error doesn't implement std::error::Error directly

    // --- Signal Handling ---
    /// Error indicating that the operation was cancelled by the user (e.g., Ctrl+C).
    #[error("Operation cancelled by user (Ctrl+C)")]
    Interrupted,

    /// Error indicating that no files were found that matched the given criteria.
    #[error("No files found matching the specified criteria.")]
    NoFilesFound,
    // --- Other specific errors can be added here ---
    // Example:
    // #[error("File processing failed for '{path}': {reason}")]
    // FileProcessingError { path: String, reason: String },
}

/// Helper function to create an `AppError::IoError` with path context.
///
/// # Arguments
/// * `source` - The original `std::io::Error`.
/// * `path` - The path associated with the error, convertible to `AsRef<std::path::Path>`.
///
/// # Returns
/// An `AppError::IoError` variant containing the path string and the source error.
pub fn io_error_with_path<P: AsRef<std::path::Path>>(source: std::io::Error, path: P) -> AppError {
    AppError::IoError {
        path: path.as_ref().display().to_string(),
        source,
    }
}

// Note: A custom `Result` type alias (`pub type Result<T> = std::result::Result<T, AppError>;`)
// is generally not needed when using `anyhow::Result` throughout the application,
// as `anyhow` handles error propagation and conversion smoothly.

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io, path::PathBuf};

    #[test]
    fn test_io_error_with_path_helper() {
        // Different test name
        let path = PathBuf::from("some/test/path.txt"); // Creates PathBuf
        let source_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let app_error = io_error_with_path(source_error, &path); // Passes &PathBuf

        match app_error {
            AppError::IoError {
                path: error_path,
                source,
            } => {
                // Use contains because canonicalization might affect the exact path string
                assert!(error_path.contains("some/test/path.txt")); // Uses contains check
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
                assert!(source.to_string().contains("File not found"));
            }
            _ => panic!("Expected AppError::IoError"),
        }

        // Test with different path representation
        let path_str = "another/path";
        let source_error_perm = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let app_error_perm = io_error_with_path(source_error_perm, path_str); // Passes &str
        match app_error_perm {
            AppError::IoError {
                path: error_path,
                source,
            } => {
                assert!(error_path.contains("another/path")); // Uses contains check
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
                assert!(source.to_string().contains("Access denied"));
            }
            _ => panic!("Expected AppError::IoError"),
        }
    }
}
