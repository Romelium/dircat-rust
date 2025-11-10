//! Defines application-specific error types.
//!
//! This module provides the `Error` enum, which categorizes common errors
//! that can occur during execution, offering more context than generic I/O or
//! `anyhow` errors.

#[cfg(feature = "git")]
use std::path::PathBuf;
use thiserror::Error;

/// A convenient result type for the `dircat` library.
pub type Result<T> = std::result::Result<T, Error>;

///
/// Errors related to invalid configuration settings or combinations.
#[derive(Error, Debug)]
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
/// use dircat::errors::{Error, ConfigError};
///
/// // Attempt to build a config with conflicting options.
/// #[cfg(feature = "clipboard")]
/// {
///     let result = ConfigBuilder::new()
///         .output_file("out.md")
///         .paste(true)
///         .build();
///
///     match result {
///         Err(Error::Config(ConfigError::Conflict { option1, option2 })) => {
///             assert_eq!(option1, "--output");
///             assert_eq!(option2, "--paste");
///         }
///         _ => panic!("Expected a Conflict error"),
///     }
/// }
/// ```
#[non_exhaustive]
pub enum ConfigError {
    /// Error for conflicting command-line options.
    #[error("Options '{option1}' and '{option2}' cannot be used simultaneously.")]
    Conflict {
        /// The name of the first conflicting option (e.g., `"--output"`).
        option1: String,
        /// The name of the second conflicting option (e.g., `"--paste"`).
        option2: String,
    },

    /// Error for an invalid value provided for an option.
    #[error("Invalid value for option '{option}': {reason}")]
    InvalidValue {
        /// The name of the option with the invalid value.
        option: String,
        /// A description of why the value is invalid.
        reason: String,
    },

    /// Error for a missing required dependency option.
    #[error("Option '{option}' requires option '{required}' to be specified.")]
    MissingDependency {
        /// The name of the option that has a missing dependency.
        option: String,
        /// The name of the required option.
        required: String,
    },

    /// Error when the git cache directory cannot be determined or created.
    #[error("Failed to determine or create git cache directory: {0}")]
    CacheDir(String),

    /// Error for an invalid regular expression.
    #[error("Invalid {name} regex: '{pattern}': {source}")]
    InvalidRegex {
        /// A description of the regex's purpose (e.g., "path", "filename").
        name: String,
        /// The invalid regex pattern string.
        pattern: String,
        /// The underlying error from the `regex` crate.
        #[source]
        source: regex::Error,
    },

    /// Error for an invalid file size format string (e.g., "1ZB").
    #[error("Invalid size format: '{0}'")]
    InvalidSizeFormat(String),
}

/// Errors related to git operations (cloning, fetching, API interaction, etc.).
#[cfg(feature = "git")]
#[derive(Error, Debug)]
///
/// # Examples
///
/// ```no_run
/// use dircat::prelude::*;
/// use dircat::errors::GitError;
///
/// # fn main() {
/// let invalid_url = "https://github.com/user/this-repo-does-not-exist.git";
/// let config = ConfigBuilder::new().input_path(invalid_url).build().unwrap();
/// let token = CancellationToken::new();
///
/// let result = dircat::run(&config, &token, None);
///
/// if let Err(Error::Git(GitError::CloneFailed { url, .. })) = result {
///     eprintln!("Failed to clone the repository from {}", url);
/// }
/// # }
/// ```
#[non_exhaustive]
pub enum GitError {
    /// Error when a git clone operation fails.
    #[error("Failed to clone repository from '{url}': {source}")]
    CloneFailed {
        /// The URL of the repository that failed to clone.
        url: String,
        /// The underlying `git2::Error`.
        #[source]
        source: git2::Error,
    },

    /// Error when a git fetch operation fails.
    #[error("Failed to fetch from remote '{remote}': {source}")]
    FetchFailed {
        /// The name of the remote that failed to fetch.
        remote: String,
        /// The underlying `git2::Error`.
        #[source]
        source: git2::Error,
    },

    /// Error for a generic failure during local repository update.
    #[error("Failed to update local repository: {0}")]
    UpdateFailed(String),

    /// Error when a specified branch, tag, or ref is not found on the remote.
    #[error("Could not find remote branch or tag named '{name}' after fetch.")]
    RefNotFound {
        /// The name of the branch or tag that was not found.
        name: String,
    },

    /// Error when downloading from the GitHub API fails.
    #[error("Failed to download from GitHub API for '{url}': {source}")]
    ApiDownloadFailed {
        /// The URL that failed to download.
        url: String,
        /// The underlying error from `reqwest` or other sources.
        #[source]
        source: anyhow::Error,
    },

    /// Error when a specified subdirectory is not found in a repository.
    #[error("Subdirectory '{path}' not found in repository '{repo}'.")]
    SubdirectoryNotFound {
        /// The relative path to the subdirectory that was not found.
        path: String,
        /// The clone URL of the repository.
        repo: String,
    },

    /// Error indicating the cached repository is corrupted.
    #[error("Cached repository at '{path}' is corrupted or invalid.")]
    CorruptedCache {
        /// The filesystem path to the corrupted cache directory.
        path: PathBuf,
    },

    /// Error when the remote's default branch cannot be determined.
    #[error("Could not determine remote's default branch: {0}")]
    DefaultBranchResolution(String),

    /// A wrapper for any other git-related `anyhow::Error`.
    #[error(transparent)]
    Generic(#[from] anyhow::Error),
}

/// Errors related to clipboard operations.
///
/// These errors can occur when initializing the clipboard provider or when
/// attempting to write content to it.
///
/// # Examples
///
/// ```
/// use dircat::errors::{Error, ClipboardError};
///
/// fn handle_error(e: Error) {
///     match e {
///         Error::Clipboard(ClipboardError::Initialization(reason)) => {
///             eprintln!("Failed to connect to clipboard service: {}", reason);
///         },
///         Error::Clipboard(ClipboardError::SetContent(reason)) => {
///             eprintln!("Failed to copy content to clipboard: {}", reason);
///         },
///         _ => eprintln!("An error occurred: {}", e),
///     }
/// }
/// ```
#[cfg(feature = "clipboard")]
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ClipboardError {
    /// Error when the clipboard provider cannot be initialized.
    #[error("Failed to initialize clipboard: {0}")]
    Initialization(String),
    /// Error when setting the clipboard content fails.
    #[error("Failed to set clipboard content: {0}")]
    SetContent(String),
}

/// The primary error type for the `dircat` library.
///
/// This enum defines categorized errors that can occur during the execution,
/// providing more context than a generic `anyhow::Error`.///
/// # Examples
///
/// ```
/// use dircat::prelude::*;
/// use std::path::Path;
///
/// fn process_directory(path: &Path) -> Result<()> {
///     let config = ConfigBuilder::new()
///         .input_path(path.to_str().unwrap())
///         .build()?;
///     let token = CancellationToken::new();
///     // In a real app, you might use run() or execute() here.
///     // For this example, we'll just return Ok.
///     // run(&config, &token, None)?;
///     Ok(())
/// }
///
/// // In your main function or error handling logic:
/// let result = process_directory(Path::new("./non_existent_dir"));
/// use dircat::errors::ConfigError;
///
/// match result {
///     Ok(()) => println!("Success!"),
///     Err(Error::NoFilesFound) => {
///         println!("Operation succeeded, but no files matched the criteria.");
///     }
///     Err(Error::Config(ConfigError::InvalidValue { option, reason })) => {
///         eprintln!("Configuration error in '{}': {}", option, reason);
///     }
///     Err(Error::Io { path, source }) => {
///         eprintln!("I/O error accessing '{}': {}", path, source);
///     }
///     Err(e) => {
///         eprintln!("An unexpected error occurred: {}", e);
///     }
/// }
/// ```
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    // --- I/O Errors ---
    /// An error occurred during file or directory access (read, write, metadata).
    #[error("I/O error accessing path '{path}': {source}")]
    Io {
        /// The path that caused the I/O error.
        path: String,
        /// The underlying `std::io::Error`.
        #[source]
        source: std::io::Error,
    },

    // --- Configuration Errors ---
    /// An error related to invalid configuration settings or combinations.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// An error related to git operations (cloning, fetching, etc.).
    #[cfg(feature = "git")]
    #[error(transparent)]
    Git(#[from] GitError),

    // --- Clipboard Errors ---
    /// An error related to clipboard operations.
    #[cfg(feature = "clipboard")]
    #[error(transparent)]
    Clipboard(#[from] ClipboardError),

    // --- Signal Handling ---
    /// The operation was cancelled by the user (e.g., via Ctrl+C).
    #[error("Operation cancelled by user (Ctrl+C)")]
    Interrupted,

    /// A wrapper for any other `anyhow::Error`, typically from internal operations.
    #[error(transparent)]
    Generic(#[from] anyhow::Error),

    /// No files were found that matched the specified criteria.
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
///
/// # Examples
///
/// ```
/// use dircat::errors::{io_error_with_path, Error};
/// use std::io;
///
/// let source_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
/// let path = "path/to/file.txt";
/// let app_err = io_error_with_path(source_err, path);
///
/// assert!(matches!(app_err, Error::Io { .. }));
/// assert!(app_err.to_string().contains("path/to/file.txt"));
/// ```
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
