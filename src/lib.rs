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
//! use dircat::{discover, process, format, ConfigBuilder};
//! use std::sync::{Arc, atomic::AtomicBool};
//! use std::fs;
//! use tempfile::tempdir;
//!
//! // 1. Set up a temporary directory with some files.
//! let temp_dir = tempdir().unwrap();
//! let temp_path_str = temp_dir.path().to_str().unwrap();
//! fs::write(temp_dir.path().join("file1.txt"), "Hello, world!").unwrap();
//! fs::write(temp_dir.path().join("file2.rs"), "fn main() { /* comment */ }").unwrap();
//!
//! // 2. Create a Config object programmatically using the builder.
//! let config = ConfigBuilder::new()
//!     .input_path(temp_path_str)
//!     .remove_comments(true)
//!     .summary(true)
//!     .build(None)
//!     .unwrap();
//!
//! // 3. Set up a stop signal for graceful interruption (e.g., by Ctrl+C).
//! let stop_signal = Arc::new(AtomicBool::new(true));
//!
//! // 4. Execute the dircat pipeline.
//! // Stage 1: Discover files.
//! let discovered_files = discover(&config, stop_signal.clone()).unwrap();
//!
//! // Stage 2: Process file content.
//! let processed_files = process(discovered_files, &config, stop_signal).unwrap();
//!
//! // Stage 3: Format the output into a buffer.
//! let mut output_buffer = Vec::new();
//! format(&processed_files, &config, &mut output_buffer).unwrap();
//!
//! // 5. Print the result.
//! let output_string = String::from_utf8(output_buffer).unwrap();
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
//! ```

// Make modules public if they contain public types used in the API
pub mod cli;
pub mod config;
pub mod constants;
pub mod core_types;
pub mod discovery;
pub mod errors;
pub mod git;
pub mod output;
pub mod processing;
pub mod progress;
pub mod signal;

// Re-export key public types for easier use as a library
pub use config::{Config, ConfigBuilder, OutputDestination};
pub use core_types::{FileCounts, FileInfo};
pub use processing::filters::{ContentFilter, RemoveCommentsFilter, RemoveEmptyLinesFilter};

use crate::errors::{Error, Result};
use crate::filtering::is_likely_text;
mod filtering;
use anyhow::Context;
use rayon::prelude::*;
use std::io::Write; // Import Write trait
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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

/// Discovers files based on the provided configuration.
///
/// This is the first stage of the pipeline. It walks the filesystem according to the
/// rules in the `Config` (respecting .gitignore, filters, etc.) and returns a
/// list of `FileInfo` structs for files that match the criteria. The content of
/// the files is not read at this stage.
///
/// # Arguments
/// * `config` - The configuration for the discovery process.
/// * `stop_signal` - An `Arc<AtomicBool>` that can be used to gracefully interrupt the process.
///
/// # Returns
/// A `Result` containing a vector of `FileInfo` structs, sorted in the final
/// processing order (normal files alphabetically, then "last" files in the specified order).
pub fn discover(config: &Config, stop_signal: Arc<AtomicBool>) -> Result<Vec<FileInfo>> {
    let (mut normal_files, mut last_files) = discovery::discover_files(config, stop_signal)?;

    // Sort normal files alphabetically by relative path
    normal_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // "last_files" are already sorted correctly by the discovery module.
    normal_files.append(&mut last_files);
    Ok(normal_files)
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
/// * `stop_signal` - An `Arc<AtomicBool>` for graceful interruption.
///
/// # Returns
/// A `Result` containing a new vector of `FileInfo` structs that have been processed
/// and filtered. Files identified as binary (and not explicitly included) will be
/// removed from this list.
pub fn process(
    files: Vec<FileInfo>,
    config: &Config,
    stop_signal: Arc<AtomicBool>,
) -> Result<Vec<FileInfo>> {
    Ok(processing::process_and_filter_files(
        files,
        config,
        stop_signal,
    )?)
}

/// Formats the processed files into the final Markdown output.
///
/// This is the final stage of the pipeline for a normal run. It takes the processed
/// `FileInfo` structs and writes the formatted output (headers, code blocks, summary)
/// to the provided writer.
///
/// # Arguments
/// * `files` - A slice of processed `FileInfo` structs from the `process` stage.
/// * `config` - The configuration for output formatting.
/// * `writer` - A mutable reference to a type that implements `std::io::Write`.
pub fn format(files: &[FileInfo], config: &Config, writer: &mut dyn Write) -> Result<()> {
    Ok(output::generate_output(files, config, writer)?)
}

/// Formats the discovered files for a dry run.
///
/// This is the final stage for a dry run. It takes a list of pre-filtered `FileInfo`
/// structs and writes a simple list of the files that would be processed to the
/// provided writer.
///
/// # Arguments
/// * `files` - A slice of discovered `FileInfo` structs from the `execute` stage.
/// * `config` - The configuration for the run.
/// * `writer` - A mutable reference to a type that implements `std::io::Write`.
pub fn format_dry_run(files: &[FileInfo], config: &Config, writer: &mut dyn Write) -> Result<()> {
    // The files are now pre-filtered by the `execute` function.
    // We just need to collect refs to pass to the writer function.
    let file_refs: Vec<&FileInfo> = files.iter().collect();
    Ok(output::dry_run::write_dry_run_output(
        writer, &file_refs, config,
    )?)
}

/// Formats the processed files into a single Markdown string.
///
/// This is a convenience wrapper around the `format` function for cases where
/// the output is needed in memory as a `String` rather than being written to a
/// stream.
///
/// # Arguments
/// * `files` - A slice of processed `FileInfo` structs from the `process` stage.
/// * `config` - The configuration for output formatting.
///
/// # Returns
/// A `Result` containing the formatted Markdown `String`.
pub fn format_to_string(files: &[FileInfo], config: &Config) -> Result<String> {
    let mut buffer = Vec::new();
    format(files, config, &mut buffer)?;
    String::from_utf8(buffer)
        .context("Failed to convert output buffer to UTF-8 string. This can happen if binary files are included and not valid UTF-8.")
        .map_err(Error::Generic)
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
/// * `stop_signal` - An `Arc<AtomicBool>` that can be used to gracefully interrupt the process.
///
/// # Returns
/// A `Result` containing a `DircatResult` on success. It returns `Err(Error::NoFilesFound)`
/// if the discovery and processing stages yield no files to output. Other errors
/// are propagated from the underlying stages.
pub fn execute(config: &Config, stop_signal: Arc<AtomicBool>) -> Result<DircatResult> {
    // Discover files based on config
    let discovered_files = discover(config, stop_signal.clone())?;

    if config.dry_run {
        // For a dry run, we just need to filter out binaries and return.
        // The content isn't processed.
        let filtered_files: Vec<FileInfo> = discovered_files
            .into_par_iter()
            .filter(|fi| {
                if config.include_binary {
                    return true;
                }
                match is_likely_text(&fi.absolute_path) {
                    Ok(is_text) => is_text,
                    Err(e) => {
                        log::warn!(
                            "Dry run: Could not check file type for '{}', skipping. Error: {}",
                            fi.absolute_path.display(),
                            e
                        );
                        false
                    }
                }
            })
            .collect();

        if filtered_files.is_empty() {
            return Err(Error::NoFilesFound);
        }
        Ok(DircatResult {
            files: filtered_files,
        })
    } else {
        // For a normal run, process the files.
        let processed_files = process(discovered_files, config, stop_signal)?;
        if processed_files.is_empty() {
            return Err(Error::NoFilesFound);
        }
        Ok(DircatResult {
            files: processed_files,
        })
    }
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
/// * `stop_signal` - An `Arc<AtomicBool>` that can be used to gracefully interrupt the process.
///
/// # Returns
/// A `Result` that is `Ok(())` on success. It returns `Err(Error::NoFilesFound)`
/// if the discovery and processing stages yield no files to output. Other errors
/// are propagated from the underlying stages.
pub fn run(config: &Config, stop_signal: Arc<AtomicBool>) -> Result<()> {
    // Execute the core logic to get the processed files.
    // This handles discovery, processing, and the NoFilesFound error.
    let result = execute(config, stop_signal)?;

    // Set up the output writer (stdout, file, or clipboard buffer)
    let writer_setup = output::writer::setup_output_writer(config)?;
    let mut writer: Box<dyn Write + Send> = writer_setup.writer;

    if config.dry_run {
        // Handle Dry Run formatting
        format_dry_run(&result.files, config, &mut writer)?;
    } else {
        // Handle Normal Run formatting
        format(&result.files, config, &mut writer)?;
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
    use std::sync::atomic::AtomicBool;
    use tempfile::tempdir;

    #[test]
    fn test_run_basic_success() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let output_file_path = temp_dir.path().join("output.md");
        fs::write(temp_dir.path().join("b.txt"), "Content B")?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .output_file(output_file_path.to_str().unwrap())
            .build(None)?;

        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = run(&config, stop_signal);

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

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .output_file(output_file_path.to_str().unwrap())
            .dry_run(true)
            .build(None)?;

        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = run(&config, stop_signal);

        // 3. Assert
        assert!(result.is_ok());

        let output_content = fs::read_to_string(&output_file_path)?;
        let expected_content =
            "\n--- Dry Run: Files that would be processed ---\n- a.rs\n- b.txt\n--- End Dry Run ---\n";
        assert_eq!(output_content, expected_content);

        Ok(())
    }

    #[test]
    fn test_run_returns_no_files_found_error() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = run(&config, stop_signal);

        // 3. Assert
        assert!(matches!(result, Err(Error::NoFilesFound)));

        Ok(())
    }

    #[test]
    fn test_run_with_filters_returns_no_files_found() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .extensions(vec!["txt".to_string()]) // Filter for .txt, but only .rs exists
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = run(&config, stop_signal);

        // 3. Assert
        assert!(matches!(result, Err(Error::NoFilesFound)));

        Ok(())
    }

    #[test]
    fn test_run_respects_stop_signal() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .build(None)?;

        // Simulate an immediate stop signal
        let stop_signal = Arc::new(AtomicBool::new(false));

        // 2. Execute
        let result = run(&config, stop_signal);

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

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .remove_comments(true) // Enable a processing step
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = execute(&config, stop_signal)?;

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

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .dry_run(true)
            .remove_comments(true) // This should be ignored in dry run
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = execute(&config, stop_signal)?;

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

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .dry_run(true)
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = execute(&config, stop_signal)?;

        // 3. Assert
        assert_eq!(result.files.len(), 1); // Binary file should be filtered out
        assert_eq!(result.files[0].relative_path.to_str(), Some("a.txt"));

        Ok(())
    }

    #[test]
    fn test_execute_returns_no_files_found() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Execute
        let result = execute(&config, stop_signal);

        // 3. Assert
        assert!(matches!(result, Err(Error::NoFilesFound)));

        Ok(())
    }

    #[test]
    fn test_format_to_string() -> anyhow::Result<()> {
        // 1. Setup
        let temp_dir = tempdir()?;
        fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

        let config = ConfigBuilder::new()
            .input_path(temp_dir.path().to_str().unwrap())
            .build(None)?;
        let stop_signal = Arc::new(AtomicBool::new(true));

        // 2. Discover and process
        let discovered_files = discover(&config, stop_signal.clone())?;
        let processed_files = process(discovered_files, &config, stop_signal)?;

        // 3. Format to string
        let output_string = format_to_string(&processed_files, &config)?;

        // 4. Assert
        let expected_content = "## File: a.rs\n```rs\nfn a() {}\n```\n";
        assert_eq!(output_string, expected_content);

        Ok(())
    }
}
