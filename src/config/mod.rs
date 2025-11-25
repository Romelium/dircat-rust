//! Defines the core `Config` struct and related types for application configuration.
//!
//! This module consolidates all the settings parsed and validated from the CLI,
//! making them available to the rest of the application in a structured and
//! type-safe manner.

use crate::processing::filters::ContentFilter;
use crate::security::SafeModeConfig;
use regex::Regex;
use std::fmt;
use std::path::PathBuf;

pub use builder::ConfigBuilder;
mod builder;
mod builder_logic;
mod parsing;
pub mod path_resolve;

/// Configuration options related to file discovery and filtering.
///
/// This struct holds all settings that control how `dircat` walks the filesystem,
/// which files it considers, and which ones it ignores based on various criteria
/// like size, extension, path, and `.gitignore` rules.
///
/// An instance of this struct is typically created as part of a `Config` object
/// via a `ConfigBuilder` rather than being constructed manually.
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
///
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new()
///     .extensions(vec!["rs".to_string(), "toml".to_string()])
///     .build()?;
///
/// // You can inspect the discovery settings after building the main config.
/// assert_eq!(config.discovery.extensions, Some(vec!["rs".to_string(), "toml".to_string()]));
/// assert_eq!(config.discovery.recursive, true); // Default value
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Maximum file size in bytes. Files larger than this are skipped.
    pub max_size: Option<u128>,
    /// Whether to recurse into subdirectories.
    pub recursive: bool,
    /// List of file extensions (lowercase) to include. If `Some`, only files with these extensions are processed.
    pub extensions: Option<Vec<String>>,
    /// List of file extensions (lowercase) to exclude. Takes precedence over `extensions`.
    pub exclude_extensions: Option<Vec<String>>,
    /// List of custom ignore patterns (gitignore syntax) provided via the `--ignore` flag.
    pub ignore_patterns: Option<Vec<String>>,
    /// List of compiled regexes to exclude files by relative path. If `Some`, any path matching one of these is skipped.
    pub exclude_path_regex: Option<Vec<Regex>>,
    /// List of compiled regexes to match against the relative file path. If `Some`, the path must match at least one.
    pub path_regex: Option<Vec<Regex>>,
    /// List of compiled regexes to match against the filename (basename). If `Some`, the filename must match at least one.
    pub filename_regex: Option<Vec<Regex>>,
    /// Whether to respect `.gitignore`, `.ignore`, and other VCS ignore files.
    pub use_gitignore: bool,
    /// Whether to skip common lockfiles (e.g., Cargo.lock, package-lock.json).
    pub skip_lockfiles: bool,
    /// List of glob patterns for files to be processed last, in the specified order.
    pub process_last: Option<Vec<String>>,
    /// If `true`, only process files matching the `process_last` patterns.
    pub only_last: bool,
    /// If `true`, enables security restrictions during discovery (e.g. no symlinks).
    pub safe_mode: bool,
    /// Maximum number of files to discover in Safe Mode to prevent iteration exhaustion.
    pub max_file_count: Option<usize>,
}

/// Configuration options related to processing file content.
///
/// This struct holds settings that control how the content of each discovered
/// file is read, interpreted, and transformed.
///
/// An instance of this struct is typically created as part of a `Config` object
/// via a `ConfigBuilder` rather than being constructed manually.
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
///
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new()
///     .include_binary(true)
///     .remove_comments(true)
///     .build()?;
///
/// // You can inspect the processing settings after building the main config.
/// assert!(config.processing.include_binary);
/// assert!(config.processing.content_filters.iter().any(|f| f.name() == "RemoveCommentsFilter"));
/// # Ok(())
/// # }
/// ```
pub struct ProcessingConfig {
    /// Whether to include files detected as binary/non-text.
    pub include_binary: bool,
    /// Whether to calculate line, character, and word counts for the summary.
    pub counts: bool,
    /// A vector of content filters to be applied sequentially to each file's content.
    pub content_filters: Vec<Box<dyn ContentFilter>>,
    /// Security configuration for processing (size limits, LFI checks).
    pub security: Option<SafeModeConfig>,
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
///
/// This struct holds settings that control the appearance of the final
/// concatenated output, such as headers, line numbers, and summaries.
///
/// An instance of this struct is typically created as part of a `Config` object
/// via a `ConfigBuilder` rather than being constructed manually.
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
///
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new()
///     .line_numbers(true)
///     .summary(true)
///     .build()?;
///
/// // You can inspect the output settings after building the main config.
/// assert!(config.output.line_numbers);
/// assert!(config.output.summary);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OutputConfig {
    /// Whether to display only the filename (basename) in the `## File:` header.
    pub filename_only_header: bool,
    /// Whether to add line numbers (`N | `) to the output.
    pub line_numbers: bool,
    /// Whether to wrap filenames in backticks (`) in headers and the summary.
    pub backticks: bool,
    /// The number of backticks to use for Markdown code fences.
    pub num_ticks: u8,
    /// Whether to print a summary list of processed files at the end. Implied by `counts`.
    pub summary: bool,
    /// Whether to display line, character, and word counts in the summary.
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
            safe_mode: false,
            max_file_count: None,
        }
    }
}

/// Represents the destination for the generated output.
///
/// This enum is used within the main `Config` struct to direct the final output.
/// It is typically determined by the `ConfigBuilder` based on whether `--output`,
/// `--paste`, or neither is specified.
///
/// # Examples
///
/// ```
/// use dircat::config::{ConfigBuilder, OutputDestination};
/// use std::path::PathBuf;
/// # use dircat::errors::Result;
///
/// # fn main() -> Result<()> {
/// // Build a config that specifies an output file.
/// let config = ConfigBuilder::new().output_file("output.md").build()?;
///
/// // You can match on the destination to handle different output strategies.
/// match config.output_destination {
///     OutputDestination::Stdout => println!("Writing to stdout."),
///     OutputDestination::File(ref path) => println!("Writing to file: {}", path.display()),
///     #[cfg(feature = "clipboard")]
///     OutputDestination::Clipboard => println!("Copying to clipboard."),
/// }
///
/// assert_eq!(config.output_destination, OutputDestination::File(PathBuf::from("output.md")));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputDestination {
    /// Write output to standard output.
    Stdout,
    /// Write output to the specified file.
    File(PathBuf),
    #[cfg(feature = "clipboard")]
    /// Copy output to the system clipboard.
    Clipboard,
}

/// The main configuration struct for a `dircat` run.
///
/// This struct holds all the settings parsed and validated from the CLI or a
/// `ConfigBuilder`, ready to be used by the core logic (discovery, processing, output).
///
/// # Examples
///
/// ```
/// # use dircat::config::{ConfigBuilder, OutputDestination};
/// # use std::path::PathBuf;
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new()
///     .input_path("./src")
///     .no_recursive(true)
///     .output_file("output.md")
///     .build()?;
///
/// assert_eq!(config.input_path, "./src");
/// assert_eq!(config.discovery.recursive, false);
/// assert_eq!(config.output_destination, OutputDestination::File(PathBuf::from("output.md")));
/// # Ok(())
/// # }
/// ```
pub struct Config {
    /// The original, unresolved path to the directory/file to process, or a git URL.
    pub input_path: String,
    /// Configuration for the file discovery stage.
    pub discovery: DiscoveryConfig,
    /// Configuration for the content processing stage.
    pub processing: ProcessingConfig,
    /// Configuration for the output formatting stage.
    pub output: OutputConfig,
    /// Specifies where the final output should be written.
    pub output_destination: OutputDestination,
    /// If `true`, performs a dry run: prints the list of files that would be processed, but not their content.
    pub dry_run: bool,
    #[cfg(feature = "git")]
    /// For git URL inputs, the specific branch, tag, or commit to check out.
    pub git_branch: Option<String>,
    #[cfg(feature = "git")]
    /// For git URL inputs, the depth for a shallow clone.
    pub git_depth: Option<u32>,
    #[cfg(feature = "git")]
    /// The path to a directory for caching cloned git repositories.
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
                safe_mode: false,
                max_file_count: None,
            },
            processing: ProcessingConfig {
                include_binary: false,
                counts: false,
                content_filters: Vec::new(),
                security: None,
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

/// Re-export public path resolution functions and related types.
#[cfg(feature = "git")]
pub use path_resolve::{determine_cache_dir, resolve_input, ResolvedInput};
#[cfg(not(feature = "git"))]
pub use path_resolve::{resolve_input, ResolvedInput};
