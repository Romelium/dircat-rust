//! Defines application-specific error types.
//!
//! This module provides the `Error` enum, which categorizes common errors
//! that can occur during execution, offering more context than generic I/O or
//! `anyhow` errors.

use anyhow;
use thiserror::Error;

/// A public result type for the `dircat` library.
pub type Result<T> = std::result::Result<T, Error>;

/// Public-facing errors for the `dircat` library.
///
/// This enum defines categorized errors that can occur during the execution,
/// providing more context than a generic `anyhow::Error`.
#[derive(Error, Debug)]
pub enum Error {
    // --- I/O Errors ---
    /// Error occurring during file or directory access (read, write, metadata).
    #[error("I/O error accessing path '{path}': {source}")]
    Io {
        /// The path that caused the I/O error.
        path: String,
        /// The underlying `std::io::Error`.
        #[source]
        source: std::io::Error,
    },

    // --- Configuration Errors ---
    /// Error related to invalid configuration settings or combinations.
    #[error("Invalid configuration: {0}")]
    Config(String),

    // --- Clipboard Errors ---
    /// Error related to clipboard operations (copying).
    #[error("Clipboard error: {0}")]
    Clipboard(String),

    // --- Signal Handling ---
    /// Error indicating that the operation was cancelled by the user (e.g., Ctrl+C).
    #[error("Operation cancelled by user (Ctrl+C)")]
    Interrupted,

    /// A wrapper for any other error type, typically from internal operations.
    #[error(transparent)]
    Generic(#[from] anyhow::Error),

    /// Error indicating that no files were found that matched the given criteria.
    #[error("No files found matching the specified criteria.")]
    NoFilesFound,
    // --- Other specific errors can be added here ---
    // Example:
    // #[error("File processing failed for '{path}': {reason}")]
    // FileProcessingError { path: String, reason: String },
}

/// Helper function to create an `Error::Io` with path context.
///
/// # Arguments
/// * `source` - The original `std::io::Error`.
/// * `path` - The path associated with the error, convertible to `AsRef<std::path::Path>`.
///
/// # Returns
/// An `Error::Io` variant containing the path string and the source error.
pub fn io_error_with_path<P: AsRef<std::path::Path>>(source: std::io::Error, path: P) -> Error {
    Error::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

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

        if let Error::Io {
            path: error_path,
            source,
        } = app_error
        {
            // Use contains because canonicalization might affect the exact path string
            assert!(error_path.contains("some/test/path.txt")); // Uses contains check
            assert_eq!(source.kind(), io::ErrorKind::NotFound);
            assert!(source.to_string().contains("File not found"));
        } else {
            panic!("Expected Error::Io");
        }

        // Test with different path representation
        let path_str = "another/path";
        let source_error_perm = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let app_error_perm = io_error_with_path(source_error_perm, path_str); // Passes &str
        if let Error::Io {
            path: error_path,
            source,
        } = app_error_perm
        {
            assert!(error_path.contains("another/path")); // Uses contains check
            assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            assert!(source.to_string().contains("Access denied"));
        } else {
            panic!("Expected Error::Io");
        }
    }
}
