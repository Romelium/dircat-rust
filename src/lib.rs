//! `dircat` is a library and command-line tool for recursively concatenating
//! directory contents into a single, well-formatted Markdown file.
//!
//! It is designed for speed, developer convenience, and seamless integration with
//! tools that consume Markdown, such as Large Language Models (LLMs) or
//! documentation systems.
//!
//! As a library, it provides a modular, three-stage pipeline:
//! 1.  **Discover**: Find all relevant files based on a rich set of filtering criteria.
//! 2.  **Process**: Read and transform file content (e.g., removing comments).
//! 3.  **Format**: Generate the final Markdown output.
//!
//! This design allows programmatic use of its components, such as using the file
//! discovery logic or content filters independently.
//!
//! # Example: Library Usage
//!
//! The following example demonstrates how to use the `dircat` library to
//! discover, process, and format files from a temporary directory.
//!
//! ```
//! use dircat::{execute};
//! use dircat::cancellation::CancellationToken;
//! use dircat::config::{self, ConfigBuilder};
//! use dircat::progress::ProgressReporter;
//! use std::fs;
//! use std::sync::Arc;
//! use tempfile::tempdir;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! // 1. Set up a temporary directory with some files.
//! let temp_dir = tempdir()?;
//! let input_path_str = temp_dir.path().to_str().unwrap();
//! fs::write(temp_dir.path().join("file1.txt"), "Hello, world!")?;
//! fs::write(temp_dir.path().join("file2.rs"), "fn main() { /* comment */ }")?;
//!
//! // 2. Create a Config object programmatically using the builder.
//! let config = ConfigBuilder::new()
//!     .input_path(input_path_str)
//!     .remove_comments(true)
//!     .summary(true)
//!     .build()?;
//!
//! // 3. Set up a cancellation token for graceful interruption.
//! let token = CancellationToken::new();
//!
//! // 4. Execute the main logic to get the processed file data. This is the recommended
//! //    approach for library use, as it guarantees a deterministically sorted result.
//! let progress: Option<Arc<dyn ProgressReporter>> = None; // No progress bar in this example
//! let dircat_result = dircat::execute(&config, &token, progress)?;
//!
//! // For more granular control, you could also use the individual stages.
//! // Note: `process` does not preserve order, so you would need to collect and sort
//! // the results yourself to match the output of `execute`.
//! // let resolved = config::resolve_input(&config.input_path, &config.git_branch, config.git_depth, &config.git_cache_path, progress)?;
//! // let discovered_files = dircat::discover(&config, &resolved, &token)?;
//! // let mut processed_files: Vec<_> = dircat::process(discovered_files, &config, &token)?.collect::<Result<_,_>>()?;
//! // processed_files.sort_by_key(|fi| (fi.is_process_last, fi.process_last_order, fi.relative_path.clone()));
//!
//! // 5. Format the output into a buffer.
//! let formatter = dircat::output::MarkdownFormatter; // The default formatter
//! let mut output_buffer: Vec<u8> = Vec::new();
//! let output_opts = dircat::OutputConfig::from(&config);
//! dircat_result.format_with(&formatter, &output_opts, &mut output_buffer)?;
//!
//! // 6. Print the result.
//! let output_string = String::from_utf8(output_buffer)?;
//! println!("{}", output_string);
//!
//! // The output would look something like this:
//! // ## File: file1.txt
//! // ```txt
//! // Hello, world!
//! // ```
//! //
//! // ## File: file2.rs
//! // ```rs
//! // fn main() {  }
//! // ```
//! //
//! // ---
//! // Processed Files: (2)
//! // - file1.txt
//! // - file2.rs
//! # Ok(())
//! # }
//! ```

// Make modules public if they contain public types used in the API
pub mod cancellation;
pub mod cli;
pub mod config;
pub mod constants;
pub mod core_types;
pub mod discovery;
pub mod errors;
#[cfg(feature = "git")]
pub mod git;
pub mod output;
pub mod processing;
pub mod progress;
pub mod signal;

/// A prelude for conveniently importing the most common types.
pub mod prelude;

// Re-export key public types for easier use as a library
pub use cancellation::CancellationToken;
pub use config::{
    Config, ConfigBuilder, DiscoveryConfig, OutputConfig, OutputDestination, ProcessingConfig,
};
pub use core_types::{FileCounts, FileInfo};
pub use discovery::discover_files;
pub use processing::{process_content, process_files, ProcessingOptions};

/// Standalone functions for file filtering and text detection.
pub use filtering::{
    check_process_last, is_file_type, is_likely_text, is_likely_text_from_buffer, is_lockfile,
    passes_extension_filters, passes_size_filter,
};
pub use output::MarkdownFormatter;
pub use output::OutputFormatter;
/// Standalone functions and traits for content processing.
pub use processing::{
    calculate_counts,
    filters::{
        remove_comments, remove_empty_lines, ContentFilter, RemoveCommentsFilter,
        RemoveEmptyLinesFilter,
    },
};

// Re-export key git utility functions for library users
#[cfg(feature = "git")]
pub use git::{
    download_directory_via_api, get_repo, is_git_url, parse_clone_url, parse_github_folder_url,
    parse_github_folder_url_with_hint, ParsedGitUrl,
};

use crate::errors::{Error, Result};
use rayon::prelude::*;
pub mod filtering;
use std::io::Write;
use std::sync::Arc; // Import Write trait

/// Represents the successful result of a dircat execution.
///
/// This struct contains the final list of files that were discovered and processed,
/// sorted in a deterministic order. It allows the library user to access the
/// processed data directly for custom formatting or analysis before writing to an output.
///
/// # Examples
///
/// ```
/// use dircat::{execute, ConfigBuilder, CancellationToken};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use std::{fs, error::Error};
/// # use tempfile::tempdir;
/// # let temp = tempdir()?;
/// # fs::write(temp.path().join("a.txt"), "content")?;
/// let config = ConfigBuilder::new().input_path(temp.path().to_str().unwrap()).build()?;
/// let token = CancellationToken::new();
///
/// let result = execute(&config, &token, None)?;
///
/// println!("Processed {} files.", result.files.len());
/// for file_info in result.files {
///     println!("- {}: {} bytes", file_info.relative_path.display(), file_info.size);
///     // You can also access file_info.processed_content, file_info.counts, etc.
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DircatResult {
    /// A vector of `FileInfo` structs, processed according to the `Config` and
    /// sorted in a deterministic order.
    /// The sorting order is: normal files alphabetically by relative path, followed by
    /// files matching `--last` patterns in the order they were specified.
    pub files: Vec<FileInfo>,
}

impl DircatResult {
    /// Formats the result using a custom output formatter.
    ///
    /// This method allows library users to provide their own implementation of the
    /// `OutputFormatter` trait to generate output in different formats (e.g., JSON, XML)
    /// without needing to reimplement the discovery and processing logic.
    ///
    /// # Arguments
    /// * `formatter` - An instance of a type that implements `OutputFormatter`.
    /// * `opts` - The configuration for output formatting.
    /// * `writer` - A mutable reference to a type that implements `std::io::Write`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dircat::{DircatResult, MarkdownFormatter, OutputConfig, execute, ConfigBuilder, CancellationToken};
    /// # use dircat::core_types::FileInfo;
    /// # use std::path::PathBuf;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // 1. Assume you have a DircatResult from a call to `execute()`.
    /// let file = FileInfo {
    ///     absolute_path: PathBuf::from("/abs/a.txt"),
    ///     relative_path: PathBuf::from("a.txt"),
    ///     size: 4,
    ///     processed_content: Some("test".to_string()),
    ///     counts: None, is_process_last: false, process_last_order: None, is_binary: false,
    /// };
    /// let result = DircatResult { files: vec![file] };
    ///
    /// // 2. Choose a formatter and configure output options.
    /// let formatter = MarkdownFormatter;
    /// let opts = OutputConfig {
    ///     filename_only_header: false, line_numbers: false, backticks: false,
    ///     num_ticks: 3, summary: false, counts: false
    /// };
    ///
    /// // 3. Format the result into a buffer.
    /// let mut buffer = Vec::new();
    /// result.format_with(&formatter, &opts, &mut buffer)?;
    ///
    /// let output = String::from_utf8(buffer)?;
    /// assert!(output.contains("## File: a.txt"));
    /// assert!(output.contains("test"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn format_with<F: OutputFormatter>(
        &self,
        formatter: &F,
        opts: &OutputConfig,
        writer: &mut dyn Write,
    ) -> Result<()> {
        Ok(formatter.format(&self.files, opts, writer)?)
    }

    /// Formats the result for a dry run using a custom output formatter.
    ///
    /// This method is intended for use when `Config::dry_run` is `true`. It allows
    /// a custom formatter to generate a preview of the files that would be processed.
    ///
    /// This method is intended for use when `Config::dry_run` is `true`. The `FileInfo`
    /// structs in the `DircatResult` will have their `processed_content` and `counts`
    /// fields set to `None`, as file content is not read during a dry run.
    ///
    /// # Arguments
    /// * `formatter` - An instance of a type that implements `OutputFormatter`.
    /// * `opts` - The configuration for output formatting.
    /// * `writer` - A mutable reference to a type that implements `std::io::Write`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use dircat::{DircatResult, MarkdownFormatter, OutputConfig, execute, ConfigBuilder, CancellationToken};
    /// # use dircat::core_types::FileInfo;
    /// # use std::path::PathBuf;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // 1. Assume you have a DircatResult from a dry run call to `execute()`.
    /// // In a dry run, `processed_content` would be None.
    /// let file = FileInfo {
    ///     absolute_path: PathBuf::from("/abs/a.txt"),
    ///     relative_path: PathBuf::from("a.txt"),
    ///     size: 4, processed_content: None, counts: None,
    ///     is_process_last: false, process_last_order: None, is_binary: false,
    /// };
    /// let result = DircatResult { files: vec![file] };
    ///
    /// // 2. Choose a formatter and configure output options.
    /// let formatter = MarkdownFormatter;
    /// let opts = OutputConfig {
    ///     filename_only_header: false, line_numbers: false, backticks: false,
    ///     num_ticks: 3, summary: false, counts: false
    /// };
    /// // 3. Format the dry run result into a buffer.
    /// let mut buffer = Vec::new();
    /// result.format_dry_run_with(&formatter, &opts, &mut buffer)?;
    ///
    /// let output = String::from_utf8(buffer)?;
    /// assert!(output.contains("--- Dry Run: Files that would be processed ---"));
    /// assert!(output.contains("- a.txt"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn format_dry_run_with<F: OutputFormatter>(
        &self,
        formatter: &F,
        opts: &OutputConfig,
        writer: &mut dyn Write,
    ) -> Result<()> {
        Ok(formatter.format_dry_run(&self.files, opts, writer)?)
    }
}

/// Discovers files based on the provided configuration.
///
/// This is the first stage of the pipeline. It walks the filesystem according to the
/// rules in the `Config` (respecting .gitignore, filters, etc.) and returns a
/// list of `FileInfo` structs for files that match the criteria. The content of
/// the files is not read at this stage.
///
/// # Arguments
/// * `config` - The configuration for the discovery process.
/// * `resolved` - The `ResolvedInput` struct containing the resolved, absolute path information.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
///
/// # Returns
/// A `Result` containing an iterator of `FileInfo` structs, sorted in the final
/// processing order (normal files alphabetically, then "last" files in the specified order).
///
/// # Examples
///
/// ```
/// use dircat::config::{self, ConfigBuilder};
/// use dircat::{discover, CancellationToken};
/// use tempfile::tempdir;
/// use std::fs;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let temp = tempdir()?;
/// fs::write(temp.path().join("a.txt"), "A")?;
/// let config = ConfigBuilder::new()
///     .input_path(temp.path().to_str().unwrap())
///     .build()?;
///
/// let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// let token = CancellationToken::new();
///
/// let discovered_files: Vec<_> = discover(&config.discovery, &resolved, &token)?.collect();
/// assert_eq!(discovered_files.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn discover(
    config: &DiscoveryConfig,
    resolved: &config::path_resolve::ResolvedInput,
    token: &CancellationToken,
) -> Result<impl Iterator<Item = FileInfo>> {
    let (mut normal_files, last_files) = discovery::discover_files(config, resolved, token)?;

    // Sort normal files alphabetically by relative path
    normal_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // "last_files" are already sorted correctly by the discovery module.
    Ok(normal_files.into_iter().chain(last_files))
}

/// Processes a list of discovered files.
///
/// This is the second stage of the pipeline. It takes an iterator of `FileInfo` structs,
/// reads the content of each file in parallel, and performs several operations:
/// - Filters out binary files (unless configured otherwise).
/// - Calculates file statistics (lines, words, characters) if requested.
/// - Applies content transformations (comment removal, empty line removal).
///
/// # Note on Ordering
///
/// This function processes files in parallel for performance and **does not
/// preserve the order** of the input iterator. The order of `FileInfo` structs
/// in the output iterator is non-deterministic.
///
/// If a deterministic order is required, you must collect the results and sort
/// them yourself. The top-level [`execute()`] function handles this automatically
/// and is the recommended approach for most library use cases.
///
/// ```
/// # use dircat::prelude::*;
/// # use dircat::config;
/// # use std::fs;
/// # use dircat::{discover, process_files};
/// # use tempfile::tempdir;
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// # let temp = tempdir()?;
/// # fs::write(temp.path().join("c.txt"), "C")?;
/// # fs::write(temp.path().join("a.rs"), "A")?;
/// # let config = ConfigBuilder::new().input_path(temp.path().to_str().unwrap()).build()?;
/// # let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// # let token = CancellationToken::new();
/// let discovered_iter = discover(&config.discovery, &resolved, &token).unwrap();
/// let processed_iter = process_files(discovered_iter, &config.processing, &token);
///
/// // The order is not guaranteed, so we must collect and sort.
/// let mut processed_files: Vec<_> = processed_iter.collect::<dircat::errors::Result<Vec<_>>>()?;
/// processed_files.sort_by_key(|fi| fi.relative_path.clone());
///
/// assert_eq!(processed_files[0].relative_path.to_str(), Some("a.rs"));
/// assert_eq!(processed_files[1].relative_path.to_str(), Some("c.txt"));
/// # Ok(())
/// # }
/// ```
///
/// # Arguments
/// * `files` - An iterator of `FileInfo` structs from the `discover` stage.
/// * `config` - The configuration for the processing stage.
/// * `token` - A `CancellationToken` for graceful interruption.
///
/// # Returns
/// An iterator that yields `Result<FileInfo>` for each successfully processed
/// file. Files identified as binary (and not explicitly included) will be
/// filtered out. I/O errors or cancellation signals will be yielded as `Err`.
///
/// # Examples
///
/// ```
/// use dircat::config::{self, ConfigBuilder};
/// use dircat::{discover, process, CancellationToken, errors::Result};
/// use tempfile::tempdir;
/// use std::fs;
///
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// let temp = tempdir()?;
/// fs::write(temp.path().join("a.rs"), "// comment\nfn main(){}")?;
/// let config = ConfigBuilder::new()
///     .input_path(temp.path().to_str().unwrap())
///     .remove_comments(true)
///     .build()?;
///
/// let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// let token = CancellationToken::new();
///
/// let discovered_iter = discover(&config.discovery, &resolved, &token).unwrap();
/// let processed_iter = process(discovered_iter, &config.processing, &token);
///
/// let processed_files: Vec<_> = processed_iter.collect::<dircat::errors::Result<_>>()?;
/// assert_eq!(processed_files.len(), 1);
/// assert_eq!(processed_files[0].processed_content.as_deref(), Some("fn main(){}"));
/// # Ok(())
/// # }
/// ```
pub fn process<'a>(
    // The iterator from `discover` is `Send`.
    files: impl Iterator<Item = FileInfo> + Send + 'a,
    config: &'a ProcessingConfig,
    token: &'a CancellationToken,
) -> impl Iterator<Item = Result<FileInfo>> {
    processing::process_files(files, config, token)
}

/// Executes the discovery and processing stages of the dircat pipeline.
///
/// This is the primary entry point for library users who want to get the processed
/// file data without immediately writing it to an output destination. It returns
/// a `DircatResult` containing the final list of `FileInfo` structs.
///
/// This function orchestrates the discovery and parallel processing stages, and
/// ensures the final list of files is deterministically sorted before returning.
/// The sorting order is: normal files alphabetically by relative path, followed by
/// files matching `--last` patterns in the order they were specified.
///
/// # Arguments
/// * `config` - The configuration for the entire run.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
/// * `progress` - An optional progress reporter for long operations like cloning.
///
/// # Examples
///
/// ```
/// use dircat::{execute, ConfigBuilder, CancellationToken};
/// use tempfile::tempdir;
/// use std::fs;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let temp = tempdir()?;
/// fs::write(temp.path().join("a.txt"), "A")?;
/// let config = ConfigBuilder::new()
///     .input_path(temp.path().to_str().unwrap())
///     .build()?;
/// let token = CancellationToken::new();
///
/// let result = execute(&config, &token, None)?;
/// assert_eq!(result.files.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn execute(
    config: &Config,
    token: &CancellationToken,
    progress: Option<Arc<dyn progress::ProgressReporter>>,
) -> Result<DircatResult> {
    // --- Path Resolution (I/O heavy part) ---
    let resolved_input = {
        #[cfg(feature = "git")]
        {
            config::resolve_input(
                &config.input_path,
                &config.git_branch,
                config.git_depth,
                &config.git_cache_path,
                progress,
            )?
        }
        #[cfg(not(feature = "git"))]
        {
            config::resolve_input(&config.input_path, &None, None, &None, progress)?
        }
    };
    // Discover files based on config
    let discovered_iter = discover(&config.discovery, &resolved_input, token)?;

    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    let mut final_files = if config.dry_run {
        // For a dry run, we just need to filter out binaries from the discovered files.
        // The content isn't processed, but we still need to read the file head to check for binary content.
        discovered_iter
            .par_bridge()
            .filter_map(|fi| {
                if token.is_cancelled() {
                    return None;
                }
                if config.processing.include_binary {
                    return Some(fi);
                }
                // Check if the file is likely text. If it is, keep it.
                match filtering::is_likely_text(&fi.absolute_path) {
                    Ok(is_text) => {
                        if is_text {
                            Some(fi)
                        } else {
                            // It's binary and we're not including binaries, so filter it out.
                            None
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "Dry run: Could not check file type for '{}', skipping. Error: {}",
                            fi.absolute_path.display(),
                            e
                        );
                        // Skip files we can't read during the check.
                        None
                    }
                }
            })
            .collect()
    } else {
        // For a normal run, process the files.
        let processed_iter = process(discovered_iter, &config.processing, token);
        processed_iter.collect::<Result<Vec<_>>>()?
    };

    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    // Re-sort the files after parallel processing, which does not preserve order.
    // The sorting criteria are:
    // 1. Normal files before "process_last" files.
    // 2. "process_last" files are sorted by the order of the glob pattern they matched.
    // 3. All other files are sorted alphabetically by relative path.
    final_files.sort_by_key(|fi| {
        (
            fi.is_process_last,
            fi.process_last_order,
            fi.relative_path.clone(),
        )
    });

    Ok(DircatResult { files: final_files })
}

/// Executes the complete dircat pipeline: discover, process, and format.
///
/// This is the primary entry point for running the tool's logic programmatically
/// in a way that mirrors the command-line execution. It orchestrates the three
/// main stages and handles output destination logic (stdout, file, or clipboard)
/// as specified in the `Config`.
///
/// For more granular control or to capture the output as a string in memory,
/// use the `execute` function to get the processed data first, then format it
/// separately.
///
/// # Arguments
/// * `config` - The configuration for the entire run.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
/// * `progress` - An optional progress reporter for long operations like cloning.
///
/// # Returns
/// A `Result` that is `Ok(())` on success. It returns `Err(Error::NoFilesFound)`
/// if the discovery and processing stages yield no files to output. Other errors
/// are propagated from the underlying stages.
///
/// # Examples
///
/// ```no_run
/// use dircat::{run, ConfigBuilder, CancellationToken};
///
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new()
///     .input_path(".")
///     .output_file("output.md")
///     .build()?;
/// let token = CancellationToken::new();
///
/// run(&config, &token, None)?;
/// # Ok(())
/// # }
/// ```
pub fn run(
    config: &Config,
    token: &CancellationToken,
    progress: Option<Arc<dyn progress::ProgressReporter>>,
) -> Result<()> {
    // Execute the core logic to get the processed files.
    let result = execute(config, token, progress)?;

    // Check if any files were found. If not, this is an error condition for a full run.
    if result.files.is_empty() {
        return Err(Error::NoFilesFound);
    }

    // Set up the output writer (stdout, file, or clipboard buffer)
    let writer_setup = output::writer::setup_output_writer(config)?;
    let mut writer: Box<dyn Write + Send> = writer_setup.writer;
    let output_config = OutputConfig::from(config);
    let formatter = MarkdownFormatter;

    if config.dry_run {
        // Handle Dry Run formatting
        result.format_dry_run_with(&formatter, &output_config, &mut writer)?;
    } else {
        // Handle Normal Run formatting
        result.format_with(&formatter, &output_config, &mut writer)?;
    }

    // Finalize output (e.g., copy to clipboard)
    Ok(output::writer::finalize_output(
        writer,
        writer_setup.clipboard_buffer,
        config,
    )?)
}
