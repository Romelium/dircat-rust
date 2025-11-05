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
mod parsing;
pub mod path_resolve;

///
/// This struct holds all the settings parsed and validated from the CLI,
/// ready to be used by the core logic (discovery, processing, output).
pub struct Config {
    /// The original, unresolved path to the directory/file to process, or a git repository URL.
    pub input_path: String,
    /// Maximum file size in bytes. Files larger than this will be skipped.
    pub max_size: Option<u128>, // Changed from u64 to u128
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
    /// Whether to include files detected as binary/non-text.
    pub include_binary: bool,
    /// Whether to skip common lockfiles (e.g., Cargo.lock, package-lock.json).
    pub skip_lockfiles: bool,
    /// A vector of content filters to be applied sequentially to each file's content.
    pub content_filters: Vec<Box<dyn ContentFilter>>,
    /// Whether to display only the filename (basename) in the `## File:` header instead of the relative path.
    pub filename_only_header: bool,
    /// Whether to add line numbers (`N | `) to the output.
    pub line_numbers: bool,
    /// Whether to wrap filenames in backticks (`) in headers and the summary.
    pub backticks: bool,
    /// Number of backticks for Markdown code fences.
    pub num_ticks: u8,
    /// Specifies where the final output should be written.
    pub output_destination: OutputDestination,
    /// Whether to print a summary list of processed files at the end. Implied by `counts`.
    pub summary: bool,
    /// Whether to calculate and display line, character, and word counts in the summary.
    pub counts: bool,
    /// List of patterns (relative paths or filenames) for files to be processed last, in the specified order.
    pub process_last: Option<Vec<String>>,
    /// If `true`, only process files matching the `process_last` patterns.
    pub only_last: bool,
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
            .field("max_size", &self.max_size)
            .field("recursive", &self.recursive)
            .field("extensions", &self.extensions)
            .field("exclude_extensions", &self.exclude_extensions)
            .field("ignore_patterns", &self.ignore_patterns)
            .field("exclude_path_regex", &self.exclude_path_regex)
            .field("path_regex", &self.path_regex)
            .field("filename_regex", &self.filename_regex)
            .field("use_gitignore", &self.use_gitignore)
            .field("include_binary", &self.include_binary)
            .field("skip_lockfiles", &self.skip_lockfiles)
            .field("content_filters", &self.content_filters)
            .field("filename_only_header", &self.filename_only_header)
            .field("line_numbers", &self.line_numbers)
            .field("backticks", &self.backticks)
            .field("num_ticks", &self.num_ticks)
            .field("output_destination", &self.output_destination)
            .field("summary", &self.summary)
            .field("counts", &self.counts)
            .field("process_last", &self.process_last)
            .field("only_last", &self.only_last)
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
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            path_regex: None,
            exclude_path_regex: None,
            filename_regex: None,
            use_gitignore: true,
            include_binary: false,
            skip_lockfiles: false,
            content_filters: Vec::new(),
            filename_only_header: false,
            line_numbers: false,
            backticks: false,
            num_ticks: 3,
            output_destination: OutputDestination::Stdout,
            summary: false,
            counts: false,
            process_last: None,
            only_last: false,
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
