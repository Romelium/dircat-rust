//! Handles the processing stage of the `dircat` pipeline.
//!
//! This module is responsible for reading file contents, detecting binary files,
//! applying content filters (like comment removal), and calculating statistics.
//! Operations are performed in parallel using Rayon for efficiency.

use crate::cancellation::CancellationToken;
use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::{io_error_with_path, Error};
use crate::filtering::is_likely_text_from_buffer;
use anyhow::Result;
use log::debug;
use rayon::prelude::*;
use std::fs;

mod content_filters;
mod counter;

use counter::calculate_counts;

/// Standalone content filtering components.
///
/// This module re-exports the `ContentFilter` trait and its concrete implementations
/// so they can be used independently of the main processing pipeline.
pub mod filters {
    // Re-export the trait, structs, and standalone functions
    pub use super::content_filters::{
        remove_comments, remove_empty_lines, ContentFilter, RemoveCommentsFilter,
        RemoveEmptyLinesFilter,
    };
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
/// * `token` - A `CancellationToken` to allow for graceful interruption.
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
    token: &CancellationToken,
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
            // Check for cancellation signal
            if token.is_cancelled() {
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
                // Apply all configured filters sequentially
                for filter in &config.content_filters {
                    processed_content = filter.apply(&processed_content);
                    debug!(
                        "Applied filter '{}' to {}",
                        filter.name(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancellation::CancellationToken;
    use crate::config::Config;
    use crate::core_types::FileInfo;
    use crate::processing::filters::{RemoveCommentsFilter, RemoveEmptyLinesFilter};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn setup_test_file(content: &[u8]) -> (tempfile::TempDir, FileInfo) {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, content).unwrap();

        let file_info = FileInfo {
            absolute_path: file_path,
            relative_path: PathBuf::from("test.rs"),
            size: content.len() as u64,
            processed_content: None,
            counts: None,
            is_process_last: false,
            process_last_order: None,
            is_binary: false,
        };

        (dir, file_info)
    }

    #[test]
    fn test_process_files_applies_filters_sequentially() -> Result<()> {
        let original_content = b"// comment\n\nfn main() {}\n";
        let (_dir, file_info) = setup_test_file(original_content);
        let token = CancellationToken::new();

        let mut config = Config::new_for_test();
        config.content_filters.push(Box::new(RemoveCommentsFilter));
        config
            .content_filters
            .push(Box::new(RemoveEmptyLinesFilter));

        let processed = process_and_filter_files(vec![file_info], &config, &token)?;

        assert_eq!(processed.len(), 1);
        // After comment removal: "\n\nfn main() {}\n" -> trim -> "fn main() {}"
        // After empty line removal: "fn main() {}" -> "fn main() {}"
        let expected_content = "fn main() {}";
        assert_eq!(
            processed[0].processed_content.as_deref(),
            Some(expected_content)
        );

        Ok(())
    }

    #[test]
    fn test_process_files_no_filters() -> Result<()> {
        let original_content = b"// comment\n\nfn main() {}\n";
        let (_dir, file_info) = setup_test_file(original_content);
        let token = CancellationToken::new();

        let config = Config::new_for_test(); // Has no filters by default
        assert!(config.content_filters.is_empty());

        let processed = process_and_filter_files(vec![file_info], &config, &token)?;

        assert_eq!(processed.len(), 1);
        // No filters, so content should be the original string
        assert_eq!(
            processed[0].processed_content.as_deref(),
            Some(std::str::from_utf8(original_content).unwrap())
        );

        Ok(())
    }

    #[test]
    fn test_process_files_skips_filters_for_binary() -> Result<()> {
        // This content will be detected as binary
        let original_content = b"binary\0content";
        let (_dir, file_info) = setup_test_file(original_content);
        let token = CancellationToken::new();

        let mut config = Config::new_for_test();
        config.include_binary = true; // We must include it to process it
        config.content_filters.push(Box::new(RemoveCommentsFilter));

        let processed = process_and_filter_files(vec![file_info], &config, &token)?;

        assert_eq!(processed.len(), 1);
        assert!(processed[0].is_binary);
        // Content should be the original (lossy converted), not filtered
        let expected_lossy = String::from_utf8_lossy(original_content);
        assert_eq!(
            processed[0].processed_content.as_deref(),
            Some(expected_lossy.as_ref())
        );

        Ok(())
    }
}
