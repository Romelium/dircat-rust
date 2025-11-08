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
//! ```no_run
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
//! // 4. Execute the main logic to get the processed file data.
//! // This is the recommended approach for library use.
//! let progress: Option<Arc<dyn ProgressReporter>> = None; // No progress bar in this example
//! let dircat_result = dircat::execute(&config, &token, progress)?;
//!
//! // For more granular control, you could also use the individual stages:
//! // let resolved = config::resolve_input(...)?;
//! // let discovered_files = dircat::discover(&config, &resolved, &token)?;
//! // let processed_files = dircat::process(discovered_files, &config, &token)?;
//!
//! // 5. Format the output into a buffer.
//! let formatter = dircat::output::MarkdownFormatter; // The default formatter
//! let mut output_buffer: Vec<u8> = Vec::new();
//! dircat_result.format_with(&formatter, &config, &mut output_buffer)?;
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

// Re-export key public types for easier use as a library
pub use cancellation::CancellationToken;
pub use config::{Config, ConfigBuilder, OutputDestination};
pub use core_types::{FileCounts, FileInfo};
pub use filtering::{is_likely_text, is_likely_text_from_buffer};
pub use output::MarkdownFormatter;
pub use processing::filters::{
    remove_comments, remove_empty_lines, ContentFilter, RemoveCommentsFilter,
    RemoveEmptyLinesFilter,
};

use crate::errors::{Error, Result};
use crate::output::OutputFormatter;
mod filtering;
use std::io::Write;
use std::sync::Arc; // Import Write trait

/// Represents the successful result of a dircat execution.
///
/// This struct contains the list of files that were discovered and processed,
/// allowing the library user to access the data directly for custom formatting
/// or analysis.
#[derive(Debug, Clone)]
pub struct DircatResult {
    /// A vector of `FileInfo` structs, sorted and processed according to the `Config`.
    /// For a dry run, `processed_content` and `counts` will be `None`.
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
    /// * `config` - The configuration for output formatting.
    /// * `writer` - A mutable reference to a type that implements `std::io::Write`.
    pub fn format_with<F: OutputFormatter>(
        &self,
        formatter: &F,
        config: &Config,
        writer: &mut dyn Write,
    ) -> Result<()> {
        Ok(formatter.format(&self.files, config, writer)?)
    }

    /// Formats the result for a dry run using a custom output formatter.
    ///
    /// # Arguments
    /// * `formatter` - An instance of a type that implements `OutputFormatter`.
    /// * `config` - The configuration for the run.
    /// * `writer` - A mutable reference to a type that implements `std::io::Write`.
    pub fn format_dry_run_with<F: OutputFormatter>(
        &self,
        formatter: &F,
        config: &Config,
        writer: &mut dyn Write,
    ) -> Result<()> {
        Ok(formatter.format_dry_run(&self.files, config, writer)?)
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
/// A `Result` containing a vector of `FileInfo` structs, sorted in the final
/// processing order (normal files alphabetically, then "last" files in the specified order).
pub fn discover(
    config: &Config,
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
/// This is the second stage of the pipeline. It takes a vector of `FileInfo` structs,
/// reads the content of each file in parallel, and performs several operations:
/// - Filters out binary files (unless configured otherwise).
/// - Calculates file statistics (lines, words, characters) if requested.
/// - Applies content transformations (comment removal, empty line removal).
///
/// # Arguments
/// * `files` - A vector of `FileInfo` structs from the `discover` stage.
/// * `config` - The configuration for the processing stage.
/// * `token` - A `CancellationToken` for graceful interruption.
///
/// # Returns
/// A `Result` containing a new vector of `FileInfo` structs that have been processed
/// and filtered. Files identified as binary (and not explicitly included) will be
/// removed from this list.
pub fn process<'a>(
    files: impl Iterator<Item = FileInfo> + 'a,
    config: &'a Config,
    token: &'a CancellationToken,
) -> impl Iterator<Item = Result<FileInfo>> + 'a {
    processing::process_and_filter_files(files, config, token)
}

/// Executes the discovery and processing stages of the dircat pipeline.
///
/// This is the primary entry point for library users who want to get the processed
/// file data without immediately writing it to an output destination. It returns
/// a `DircatResult` containing the final list of `FileInfo` structs, which can
/// then be passed to `format` or `format_dry_run` for rendering.
///
/// # Arguments
/// * `config` - The configuration for the entire run.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
/// * `progress` - An optional progress reporter for long operations like cloning.
///
/// # Returns
/// A `Result` containing a `DircatResult` on success. The `files` vector within the
/// result will be empty if no files matched the criteria. Other errors are propagated
/// from the underlying stages.
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
    let discovered_iter = discover(config, &resolved_input, token)?;

    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    let final_files = if config.dry_run {
        // For a dry run, we just need to filter out binaries from the discovered files.
        // The content isn't processed, but we still need to read the file head to check for binary content.
        discovered_iter
            .filter_map(|fi| {
                if token.is_cancelled() {
                    return None;
                }
                if config.include_binary {
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
        let processed_iter = process(discovered_iter, config, token);
        processed_iter.collect::<Result<Vec<_>>>()?
    };

    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

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
///
/// # Returns
/// A `Result` that is `Ok(())` on success. It returns `Err(Error::NoFilesFound)`
/// if the discovery and processing stages yield no files to output. Other errors
/// are propagated from the underlying stages.
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
    let formatter = MarkdownFormatter;

    if config.dry_run {
        // Handle Dry Run formatting
        result.format_dry_run_with(&formatter, config, &mut writer)?;
    } else {
        // Handle Normal Run formatting
        result.format_with(&formatter, config, &mut writer)?;
    }

    // Finalize output (e.g., copy to clipboard)
    Ok(output::writer::finalize_output(
        writer,
        writer_setup.clipboard_buffer,
        config,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Error;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_run_basic_success() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let output_file_path = temp_dir.path().join("output.md");
        fs::write(temp_dir.path().join("b.txt"), "Content B")?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .output_file(output_file_path.to_str().unwrap())
            .build()?;

        let token = CancellationToken::new();

        // 2. Execute
        let result = run(&config, &token, None);

        // 3. Assert
        assert!(result.is_ok());

        let output_content = fs::read_to_string(&output_file_path)?;
        // discover sorts files alphabetically
        let expected_content =
            "## File: a.rs\n```rs\nfn a() {}\n```\n\n## File: b.txt\n```txt\nContent B\n```\n";
        assert_eq!(output_content, expected_content);

        Ok(())
    }

    #[test]
    fn test_run_dry_run_success() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let output_file_path = temp_dir.path().join("output.md");
        fs::write(temp_dir.path().join("b.txt"), "Content B")?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .output_file(output_file_path.to_str().unwrap())
            .dry_run(true)
            .build()?;

        let token = CancellationToken::new();

        // 2. Execute
        let result = run(&config, &token, None);

        // 3. Assert
        assert!(result.is_ok());

        let output_content = fs::read_to_string(&output_file_path)?;
        let expected_content =
            "\n--- Dry Run: Files that would be processed ---\n- a.rs\n- b.txt\n--- End Dry Run ---\n";
        assert_eq!(output_content, expected_content);

        Ok(())
    }

    #[test]
    fn test_execute_returns_empty_vec_when_no_files_found() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let input_path_str = temp_dir.path().to_str().unwrap();
        let config = ConfigBuilder::new().input_path(input_path_str).build()?;
        let token = CancellationToken::new();
        // 2. Execute
        let result = execute(&config, &token, None)?;

        // 3. Assert
        assert!(result.files.is_empty());

        Ok(())
    }

    #[test]
    fn test_run_returns_no_files_found_error() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let input_path_str = temp_dir.path().to_str().unwrap();
        let config = ConfigBuilder::new().input_path(input_path_str).build()?;
        let token = CancellationToken::new();

        // 2. Execute
        let result = run(&config, &token, None);

        // 3. Assert
        assert!(matches!(result, Err(Error::NoFilesFound)));

        Ok(())
    }

    #[test]
    fn test_run_with_filters_returns_no_files_found() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .extensions(vec!["txt".to_string()]) // Filter for .txt, but only .rs exists
            .build()?;
        let token = CancellationToken::new();

        // 2. Execute
        let result = run(&config, &token, None);

        // 3. Assert
        assert!(matches!(result, Err(Error::NoFilesFound)));

        Ok(())
    }

    #[test]
    fn test_run_respects_stop_signal() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let input_path_str = temp_dir.path().to_str().unwrap();
        let config = ConfigBuilder::new().input_path(input_path_str).build()?;

        // Simulate an immediate cancellation
        let token = CancellationToken::new();
        token.cancel();

        // 2. Execute
        let result = run(&config, &token, None);

        // 3. Assert
        // The error should be `Interrupted` because the discovery loop checks the signal.
        assert!(matches!(result, Err(Error::Interrupted)));

        Ok(())
    }

    // --- Rests for execute() ---

    #[test]
    fn test_execute_normal_run_returns_processed_files() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() { /* comment */ }")?;
        fs::write(temp_dir.path().join("b.txt"), "Content B")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .remove_comments(true) // Enable a processing step
            .build()?;
        let token = CancellationToken::new();

        // 2. Execute
        let result = execute(&config, &token, None)?;

        // 3. Assert
        assert_eq!(result.files.len(), 2);
        // Files are sorted alphabetically by discover()
        let file_a = &result.files[0];
        let file_b = &result.files[1];

        assert_eq!(file_a.relative_path.to_str(), Some("a.rs"));
        assert_eq!(file_a.processed_content, Some("fn a() {  }".to_string())); // Comment removed

        assert_eq!(file_b.relative_path.to_str(), Some("b.txt"));
        assert_eq!(file_b.processed_content, Some("Content B".to_string()));

        Ok(())
    }

    #[test]
    fn test_execute_dry_run_returns_unprocessed_files() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() { /* comment */ }")?;
        fs::write(temp_dir.path().join("b.txt"), "Content B")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .dry_run(true)
            .remove_comments(true) // This should be ignored in dry run
            .build()?;
        let token = CancellationToken::new();

        // 2. Execute
        let result = execute(&config, &token, None)?;

        // 3. Assert
        assert_eq!(result.files.len(), 2);
        let file_a = &result.files[0];
        let file_b = &result.files[1];

        assert_eq!(file_a.relative_path.to_str(), Some("a.rs"));
        assert!(file_a.processed_content.is_none()); // Content not read in dry run

        assert_eq!(file_b.relative_path.to_str(), Some("b.txt"));
        assert!(file_b.processed_content.is_none());

        Ok(())
    }

    #[test]
    fn test_execute_dry_run_filters_binary_files() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.txt"), "text file")?;
        fs::write(temp_dir.path().join("b.bin"), b"binary\0data")?;

        fs::write(temp_dir.path().join("a.txt"), "text file")?;
        fs::write(temp_dir.path().join("b.bin"), b"binary\0data")?;

        let input_path_str = temp_dir.path().to_str().unwrap();

        let config = ConfigBuilder::new()
            .input_path(input_path_str)
            .dry_run(true)
            .build()?;
        let token = CancellationToken::new();

        // 2. Execute
        let result = execute(&config, &token, None)?;

        // 3. Assert
        assert_eq!(result.files.len(), 1); // Binary file should be filtered out
        assert_eq!(result.files[0].relative_path.to_str(), Some("a.txt"));

        Ok(())
    }

    #[test]
    fn test_format_to_string() -> anyhow::Result<()> {
        // This test now verifies the new recommended way of getting a string.
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let input_path_str = temp_dir.path().to_str().unwrap();
        let config = ConfigBuilder::new().input_path(input_path_str).build()?;
        let token = CancellationToken::new();

        // 2. Execute to get the result
        let result = execute(&config, &token, None)?;

        // 3. Format to a buffer and then to a string
        let mut buffer = Vec::new();
        result.format_with(&MarkdownFormatter, &config, &mut buffer)?;
        let output_string = String::from_utf8(buffer)?;

        // 4. Assert
        let expected_content = "## File: a.rs\n```rs\nfn a() {}\n```\n";
        assert_eq!(output_string, expected_content);

        Ok(())
    }
}
