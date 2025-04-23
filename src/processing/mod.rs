// src/processing/mod.rs

use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::AppError;
use anyhow::Result;
use log::debug;
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod content_filters;
mod content_reader;
mod counter;

// Use the new robust comment removal function
use content_filters::{remove_comments, remove_empty_lines};
use content_reader::read_file_content;
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

            debug!("Processing file: {}", file_info.absolute_path.display());

            // --- Read File Content ---
            let original_content = read_file_content(&file_info.absolute_path)?;

            // --- Calculate Counts (on original content) ---
            if config.counts {
                file_info.counts = Some(calculate_counts(&original_content));
                debug!(
                    "Calculated counts for {}: {:?}",
                    file_info.relative_path.display(),
                    file_info.counts
                );
            }

            // --- Apply Content Filters ---
            let mut processed_content = original_content; // Start with original

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

            // Store the final processed content
            file_info.processed_content = Some(processed_content);

            Ok(())
        })?; // The `?` propagates the first error encountered in the parallel loop

    debug!("Finished processing batch of {} files.", files.len());
    Ok(())
}
