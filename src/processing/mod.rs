// src/processing/mod.rs

use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::{io_error_with_path, AppError};
use anyhow::{Context, Result};
use log::debug;
use rayon::prelude::*;
use std::fs; // Import fs for read
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod content_filters;
mod content_reader;
mod counter;

// Use the new robust comment removal function
use content_filters::{remove_comments, remove_empty_lines};
use content_reader::read_file_content; // Still used for text files
use counter::calculate_counts;

/// Reads and processes the content of a batch of discovered files based on config.
/// Operates in parallel using Rayon. Updates FileInfo structs in place.
pub fn process_batch(
    files: &mut [FileInfo], // Takes mutable slice to update content/counts
    config: &Config,
    stop_signal: Arc<AtomicBool>,
) -> Result<()> {
    if files.is_empty() {
        debug!("No files in this batch to process.");
        return Ok(());
    }
    debug!("Starting parallel processing for {} files.", files.len());

    files
        .par_iter_mut()
        .try_for_each(|file_info| -> Result<()> {
            // Check for Ctrl+C signal before processing each file
            // Corrected Check: Interrupt if stop_signal is FALSE
            if !stop_signal.load(Ordering::Relaxed) {
                return Err(AppError::Interrupted.into());
            }

            debug!(
                "Processing file: {} (Binary: {})",
                file_info.absolute_path.display(),
                file_info.is_binary
            );

            // --- Read File Content (adapt for binary) ---
            let original_content_str: String;
            let original_content_bytes: Vec<u8>; // Store raw bytes for binary counts

            if file_info.is_binary {
                // Read as bytes for binary files
                original_content_bytes = fs::read(&file_info.absolute_path)
                    .map_err(|e| io_error_with_path(e, &file_info.absolute_path))
                    .with_context(|| {
                        format!(
                            "Failed to read binary file content: {}",
                            file_info.absolute_path.display()
                        )
                    })?;
                // Convert to lossy string for processing/output
                original_content_str = String::from_utf8_lossy(&original_content_bytes).to_string();
                debug!(
                    "Read {} bytes from binary file {}",
                    original_content_bytes.len(),
                    file_info.relative_path.display()
                );
            } else {
                // Read as string for text files
                original_content_str = read_file_content(&file_info.absolute_path)?;
                original_content_bytes = original_content_str.as_bytes().to_vec(); // Get bytes for consistent counting
                debug!(
                    "Read {} bytes from text file {}",
                    original_content_bytes.len(),
                    file_info.relative_path.display()
                );
            }

            // --- Calculate Counts ---
            // Always calculate counts if requested, but adapt for binary.
            if config.counts {
                if file_info.is_binary {
                    // For binary, only character (byte) count is meaningful.
                    // Lines/words are typically 0 or misleading.
                    file_info.counts = Some(crate::core_types::FileCounts {
                        lines: 0, // Or maybe 1 if not empty? Let's stick to 0 for binary.
                        characters: original_content_bytes.len(),
                        words: 0,
                    });
                } else {
                    // For text, calculate all counts from the original string content.
                    file_info.counts = Some(calculate_counts(&original_content_str));
                }
                debug!(
                    "Calculated counts for {}: {:?}",
                    file_info.relative_path.display(),
                    file_info.counts
                );
            }

            // --- Apply Content Filters (only if not binary) ---
            let mut processed_content = original_content_str; // Start with original (or lossy) string

            if !file_info.is_binary {
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

            Ok(())
        })?; // The `?` propagates the first error encountered in the parallel loop

    debug!("Finished processing batch of {} files.", files.len());
    Ok(())
}
