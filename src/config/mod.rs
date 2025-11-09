//! Defines the core `Config` struct and related types for application configuration.
//!
//! This module consolidates all the settings parsed and validated from the CLI,
//! making them available to the rest of the application in a structured and
//! type-safe manner.

use crate::processing::filters::ContentFilter;
use regex::Regex;
use std::fmt;
use std::path::PathBuf;

pub use builder::ConfigBuilder;
mod builder;
mod builder_logic;
mod parsing;
pub mod path_resolve;

/// Configuration options related to file discovery and filtering.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Maximum file size in bytes. Files larger than this will be skipped.
    pub max_size: Option<u128>,
    /// Whether to recurse into subdirectories.
    pub recursive: bool,
    /// List of file extensions (lowercase) to include. If `Some`, only files with these extensions are processed.
    pub extensions: Option<Vec<String>>,
    /// List of file extensions (lowercase) to exclude. Takes precedence over `extensions`.
    pub exclude_extensions: Option<Vec<String>>,
    /// List of custom ignore patterns (gitignore syntax) provided via `-i`.
    pub ignore_patterns: Option<Vec<String>>,
    /// List of compiled regular expressions to exclude files by full path. If `Some`, any path matching one of these is skipped.
    pub exclude_path_regex: Option<Vec<Regex>>,
    /// List of compiled regular expressions to match against the full file path. If `Some`, the path must match at least one.
    pub path_regex: Option<Vec<Regex>>,
    /// List of compiled regular expressions to match against the filename (basename). If `Some`, the filename must match at least one.
    pub filename_regex: Option<Vec<Regex>>,
    /// Whether to respect `.gitignore`, `.ignore`, and other VCS ignore files.
    pub use_gitignore: bool,
    /// Whether to skip common lockfiles (e.g., Cargo.lock, package-lock.json).
    pub skip_lockfiles: bool,
    /// List of patterns (relative paths or filenames) for files to be processed last, in the specified order.
    pub process_last: Option<Vec<String>>,
    /// If `true`, only process files matching the `process_last` patterns.
    pub only_last: bool,
}

/// Configuration options related to processing file content.
pub struct ProcessingConfig {
    /// Whether to include files detected as binary/non-text.
    pub include_binary: bool,
    /// Whether to calculate and display line, character, and word counts in the summary.
    pub counts: bool,
    /// A vector of content filters to be applied sequentially to each file's content.
    pub content_filters: Vec<Box<dyn ContentFilter>>,
}

// Custom Debug implementation for ProcessingConfig
impl fmt::Debug for ProcessingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessingConfig")
            .field("include_binary", &self.include_binary)
            .field("counts", &self.counts)
            .field("content_filters", &self.content_filters)
            .finish()
    }
}

/// Configuration options related to formatting the final output.
#[derive(Debug, Clone, Copy)]
pub struct OutputConfig {
    /// Whether to display only the filename (basename) in the `## File:` header instead of the relative path.
    pub filename_only_header: bool,
    /// Whether to add line numbers (`N | `) to the output.
    pub line_numbers: bool,
    /// Whether to wrap filenames in backticks (`) in headers and the summary.
    pub backticks: bool,
    /// Number of backticks for Markdown code fences.
    pub num_ticks: u8,
    /// Whether to print a summary list of processed files at the end. Implied by `counts`.
    pub summary: bool,
    /// Whether to calculate and display line, character, and word counts in the summary.
    pub counts: bool,
}

impl DiscoveryConfig {
    #[doc(hidden)]
    pub fn default_for_test() -> Self {
        Self {
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            path_regex: None,
            exclude_path_regex: None,
            filename_regex: None,
            use_gitignore: true,
            skip_lockfiles: false,
            process_last: None,
            only_last: false,
        }
    }
}
///
/// This struct holds all the settings parsed and validated from the CLI,
/// ready to be used by the core logic (discovery, processing, output).
pub struct Config {
    /// The original, unresolved path to the directory/file to process, or a git repository URL.
    pub input_path: String,
    /// Maximum file size in bytes. Files larger than this will be skipped.
    /// Configuration for the discovery stage.
    pub discovery: DiscoveryConfig,
    /// Configuration for the processing stage.
    pub processing: ProcessingConfig,
    /// Configuration for the output stage.
    pub output: OutputConfig,
    /// Specifies where the final output should be written.
    pub output_destination: OutputDestination,
    /// If `true`, perform a dry run: print the list of files that would be processed, but not their content.
    pub dry_run: bool,
    #[cfg(feature = "git")]
    /// The specific git branch to clone.
    pub git_branch: Option<String>,
    #[cfg(feature = "git")]
    /// The depth for a shallow git clone.
    pub git_depth: Option<u32>,
    #[cfg(feature = "git")]
    /// The original, unresolved path to the directory for caching cloned git repositories.
    pub git_cache_path: Option<String>,
}

// Custom Debug implementation for Config, as Box<dyn ContentFilter> does not implement Debug.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("Config");
        builder
            .field("input_path", &self.input_path)
            .field("discovery", &self.discovery)
            .field("processing", &self.processing)
            .field("output", &self.output)
            .field("output_destination", &self.output_destination)
            .field("dry_run", &self.dry_run);

        #[cfg(feature = "git")]
        {
            builder
                .field("git_branch", &self.git_branch)
                .field("git_depth", &self.git_depth)
                .field("git_cache_path", &self.git_cache_path);
        }

        builder.finish()
    }
}
impl Config {
    /// Creates a default `Config` for testing purposes.
    ///
    /// This function is hidden from public documentation and is intended for
    /// use in tests and doc tests only.
    #[doc(hidden)]
    pub fn new_for_test() -> Self {
        Self {
            input_path: ".".to_string(),
            discovery: DiscoveryConfig {
                max_size: None,
                recursive: true,
                extensions: None,
                exclude_extensions: None,
                ignore_patterns: None,
                path_regex: None,
                exclude_path_regex: None,
                filename_regex: None,
                use_gitignore: true,
                skip_lockfiles: false,
                process_last: None,
                only_last: false,
            },
            processing: ProcessingConfig {
                include_binary: false,
                counts: false,
                content_filters: Vec::new(),
            },
            output: OutputConfig {
                filename_only_header: false,
                line_numbers: false,
                backticks: false,
                num_ticks: 3,
                summary: false,
                counts: false,
            },
            output_destination: OutputDestination::Stdout,
            dry_run: false,
            #[cfg(feature = "git")]
            git_branch: None,
            #[cfg(feature = "git")]
            git_depth: None,
            #[cfg(feature = "git")]
            git_cache_path: None,
        }
    }
}

/// Represents the destination for the generated output.
#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum OutputDestination {
    /// Write to standard output.
    Stdout,
    /// Write to the specified file path.
    File(PathBuf),
    #[cfg(feature = "clipboard")]
    /// Copy the output to the system clipboard (requires the `clipboard` feature).
    Clipboard,
}

/// Re-export the public path resolution function and its related types.
#[cfg(feature = "git")]
pub use path_resolve::{determine_cache_dir, resolve_input, ResolvedInput};
#[cfg(not(feature = "git"))]
pub use path_resolve::{resolve_input, ResolvedInput};
