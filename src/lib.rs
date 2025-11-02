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
pub mod signal;

// Re-export key public types for easier use as a library
pub use config::{Config, ConfigBuilder, OutputDestination};
pub use core_types::{FileCounts, FileInfo};

use crate::filtering::is_likely_text;
mod filtering;
use anyhow::Result;
use rayon::prelude::*;
use std::io::Write; // Import Write trait
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
    processing::process_and_filter_files(files, config, stop_signal)
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
    output::generate_output(files, config, writer)
}

/// Formats the discovered files for a dry run.
///
/// This is the final stage for a dry run. It performs a quick, parallel check for
/// binary files and then writes a simple list of the files that would be processed
/// to the provided writer.
///
/// # Arguments
/// * `files` - A slice of discovered `FileInfo` structs from the `discover` stage.
/// * `config` - The configuration for the run.
/// * `writer` - A mutable reference to a type that implements `std::io::Write`.
pub fn format_dry_run(files: &[FileInfo], config: &Config, writer: &mut dyn Write) -> Result<()> {
    let filtered_files: Vec<&FileInfo> = files
        .par_iter()
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

    output::dry_run::write_dry_run_output(writer, &filtered_files, config)
}
