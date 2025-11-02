//! Handles the processing stage of the `dircat` pipeline.
//!
//! This module is responsible for reading file contents, detecting binary files,
//! applying content filters (like comment removal), and calculating statistics.
//! Operations are performed in parallel using Rayon for efficiency.

use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::{io_error_with_path, Error};
use crate::filtering::is_likely_text_from_buffer;
use anyhow::Result;
use log::debug;
use rayon::prelude::*;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod content_filters;
mod counter;

use content_filters::{remove_comments, remove_empty_lines};
use counter::calculate_counts;

/// Standalone content filtering functions.
///
/// This module re-exports content transformation functions so they can be used
/// independently of the main processing pipeline.
pub mod filters {
    pub use super::content_filters::{remove_comments, remove_empty_lines};
}

/// Reads and processes the content of a batch of discovered files based on config.
///
/// This function takes ownership of a `Vec<FileInfo>`, reads each file's content,
/// and performs several operations in parallel:
/// - Detects and filters out binary files (unless `--include-binary` is used).
/// - Calculates counts (lines, words, characters) if requested.
/// - Applies content transformations like comment and empty line removal.
///
/// # Arguments
/// * `files` - A `Vec<FileInfo>` from the discovery stage.
/// * `config` - The application configuration.
/// * `stop_signal` - An `Arc<AtomicBool>` to allow for graceful interruption.
///
/// # Returns
/// A `Result` containing a new `Vec<FileInfo>` with processed content. Files that
/// were filtered out (e.g., binaries) are not included in the returned vector.
///
/// # Errors
/// Returns an error if file I/O fails for any file or if the operation is interrupted.
pub fn process_and_filter_files(
    files: Vec<FileInfo>,
    config: &Config,
    stop_signal: Arc<AtomicBool>,
) -> Result<Vec<FileInfo>> {
    if files.is_empty() {
        debug!("No files in this batch to process.");
        return Ok(Vec::new());
    }
    let initial_file_count = files.len();
    debug!(
        "Starting parallel processing for {} files.",
        initial_file_count
    );

    let processed_files: Vec<FileInfo> = files
        .into_par_iter()
        .filter_map(|mut file_info| {
            // Check for Ctrl+C signal
            if !stop_signal.load(Ordering::Relaxed) {
                return Some(Err(Error::Interrupted.into()));
            }

            debug!("Processing file: {}", file_info.absolute_path.display());

            // --- 1. Read File Content (once) ---
            let content_bytes = match fs::read(&file_info.absolute_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    let app_err = io_error_with_path(e, &file_info.absolute_path);
                    // The .context() call was incorrect as anyhow::Error doesn't have it.
                    // The AppError itself provides sufficient context (path + source error).
                    return Some(Err(anyhow::Error::new(app_err)));
                }
            };

            // --- 2. Perform Binary Check ---
            let is_binary = !is_likely_text_from_buffer(&content_bytes);
            file_info.is_binary = is_binary;

            // --- 3. Filter Based on Binary Check ---
            if is_binary && !config.include_binary {
                debug!(
                    "Skipping binary file: {}",
                    file_info.relative_path.display()
                );
                return None; // Filter out this file by returning None
            }

            // --- 4. Process Content ---
            let original_content_str = String::from_utf8_lossy(&content_bytes).to_string();

            // --- Calculate Counts ---
            if config.counts {
                if is_binary {
                    file_info.counts = Some(crate::core_types::FileCounts {
                        lines: 0,
                        characters: content_bytes.len(),
                        words: 0,
                    });
                } else {
                    file_info.counts = Some(calculate_counts(&original_content_str));
                }
                debug!(
                    "Calculated counts for {}: {:?}",
                    file_info.relative_path.display(),
                    file_info.counts
                );
            }

            // --- Apply Content Filters ---
            let mut processed_content = original_content_str;
            if !is_binary {
                if config.remove_comments {
                    // Use the robust comment removal function
                    processed_content = remove_comments(&processed_content);
                    debug!(
                        "Applied comment removal to {}",
                        file_info.relative_path.display()
                    );
                }

                if config.remove_empty_lines {
                    processed_content = remove_empty_lines(&processed_content);
                    debug!(
                        "Applied empty line removal to {}",
                        file_info.relative_path.display()
                    );
                }
            } else {
                debug!(
                    "Skipping content filters for binary file {}",
                    file_info.relative_path.display()
                );
            }

            // Store the final processed content
            file_info.processed_content = Some(processed_content);

            Some(Ok(file_info))
        })
        .collect::<Result<Vec<_>>>()?;

    debug!("Finished processing batch of {} files.", initial_file_count);
    Ok(processed_files)
}
