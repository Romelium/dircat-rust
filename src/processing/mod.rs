//! Handles the processing stage of the `dircat` pipeline.
//!
//! This module is responsible for reading file contents, detecting binary files,
//! applying content filters (like comment removal), and calculating statistics.
//! Operations are performed in parallel using Rayon for efficiency.

use crate::cancellation::CancellationToken;
use crate::config::{Config, ProcessingConfig};
use crate::core_types::FileInfo;
use crate::errors::{io_error_with_path, Error, Result};
use crate::filtering::is_likely_text_from_buffer;
use log::debug;
use rayon::prelude::*;

use crate::core_types::FileContent;
pub mod counter;
pub mod filters;
pub use counter::calculate_counts;
use filters::ContentFilter;
use std::fs;

/// A struct holding borrowed configuration relevant to the processing stage.
///
/// This is created from the main `Config` to pass only the necessary options
/// to the processing functions, improving modularity.
#[doc(hidden)] // Internal detail, not for public library use
#[derive(Debug, Clone, Copy)]
pub struct ProcessingOptions<'a> {
    /// Whether to include files detected as binary/non-text.
    pub include_binary: bool,
    /// Whether to calculate and display line, character, and word counts in the summary.
    pub counts: bool,
    /// A vector of content filters to be applied sequentially to each file's content.
    pub content_filters: &'a [Box<dyn ContentFilter>],
}

impl<'a> From<&'a Config> for ProcessingOptions<'a> {
    fn from(config: &'a Config) -> Self {
        Self {
            include_binary: config.processing.include_binary,
            counts: config.processing.counts,
            content_filters: &config.processing.content_filters,
        }
    }
}

/// Processes file content that has already been read into memory.
///
/// This function is decoupled from filesystem I/O. It takes an iterator of
/// `FileContent` structs and performs several operations in parallel:
/// - Detects and filters out binary files (unless `--include-binary` is used).
/// - Calculates counts (lines, words, characters) if requested.
/// - Applies content transformations like comment and empty line removal.
///
/// # Returns
/// An iterator that yields a `Result<FileInfo>` for each successfully processed
/// file. Binary files are filtered out unless `opts.include_binary` is true.
///
/// # Examples
///
/// ```
/// use dircat::processing::{process_content, ProcessingOptions};
/// use dircat::core_types::FileContent;
/// use dircat::CancellationToken;
///
/// # fn main() -> dircat::errors::Result<()> {
/// let files_content = vec![FileContent {
///     relative_path: "a.rs".into(),
///     content: b"fn main() {}".to_vec(),
///     is_process_last: false, process_last_order: None,
/// }];
/// let opts = ProcessingOptions { include_binary: false, counts: false, content_filters: &[] };
/// let token = CancellationToken::new();
/// let processed_files: Vec<_> = process_content(files_content.into_iter(), opts, &token).collect::<dircat::errors::Result<_>>()?;
/// println!("Processed {} files from memory.", processed_files.len());
/// # Ok(())
/// # }
/// ```
pub fn process_content<'a>(
    files_content: impl Iterator<Item = FileContent> + Send + 'a,
    opts: ProcessingOptions<'a>,
    token: &'a CancellationToken,
) -> impl Iterator<Item = Result<FileInfo>> {
    let results: Vec<Result<FileInfo>> = files_content
        .par_bridge()
        .filter_map(move |file_content| {
            if token.is_cancelled() {
                return Some(Err(Error::Interrupted));
            }

            debug!(
                "Processing content for: {}",
                file_content.relative_path.display()
            );

            let content_bytes = &file_content.content;

            // --- Perform Binary Check ---
            let is_binary = !is_likely_text_from_buffer(content_bytes);

            // --- Filter Based on Binary Check ---
            if is_binary && !opts.include_binary {
                debug!(
                    "Skipping binary content: {}",
                    file_content.relative_path.display()
                );
                return None;
            }

            // --- Process Content ---
            let original_content_str = String::from_utf8_lossy(content_bytes).to_string();

            let mut file_info = FileInfo {
                // Since we are decoupled from the FS, absolute_path is the same as relative.
                absolute_path: file_content.relative_path.clone(),
                relative_path: file_content.relative_path,
                size: content_bytes.len() as u64,
                processed_content: None,
                counts: None,
                is_process_last: file_content.is_process_last,
                process_last_order: file_content.process_last_order,
                is_binary,
            };

            // --- Calculate Counts ---
            if opts.counts {
                file_info.counts = Some(if is_binary {
                    crate::core_types::FileCounts {
                        lines: 0,
                        characters: content_bytes.len(),
                        words: 0,
                    }
                } else {
                    calculate_counts(&original_content_str)
                });
            }

            // --- Apply Content Filters ---
            file_info.processed_content = Some(if !is_binary {
                opts.content_filters
                    .iter()
                    .fold(original_content_str, |acc, filter| filter.apply(&acc))
            } else {
                original_content_str
            });

            Some(Ok(file_info))
        })
        .collect();
    results.into_iter()
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
pub(crate) fn process_and_filter_files_internal<'a>(
    files: impl ParallelIterator<Item = FileInfo> + 'a,
    config: &'a ProcessingConfig,
    token: &'a CancellationToken,
) -> impl ParallelIterator<Item = Result<FileInfo>> + 'a {
    files.filter_map(move |mut file_info| {
        // The closure captures `config` and `token` which have lifetime 'a
        // Check for cancellation signal. If cancelled, return an error for this item
        // and subsequent calls will be filtered out by the was_cancelled flag.
        if token.is_cancelled() {
            return Some(Err(Error::Interrupted));
        }

        debug!("Processing file: {}", file_info.absolute_path.display());

        // --- 1. Read File Content (once) ---
        let content_bytes = match fs::read(&file_info.absolute_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                let app_err = io_error_with_path(e, &file_info.absolute_path);
                return Some(Err(app_err));
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
}

/// Processes a list of discovered files using a specific `ProcessingConfig`.
///
/// This is the primary, granular entry point for the processing stage. It allows for
/// creating a `ProcessingConfig` and passing it in.
///
/// # Arguments
/// * `files` - An iterator of `FileInfo` structs from the discovery stage.
/// * `config` - The processing-specific configuration.
/// * `token` - A `CancellationToken` for graceful interruption.
///
/// # Examples
///
/// ```
/// use dircat::config::{self, ConfigBuilder};
/// use dircat::{discover, process_files, CancellationToken};
///
/// # fn main() -> dircat::errors::Result<()> {
/// let config = ConfigBuilder::new().input_path(".").remove_comments(true).build()?;
/// let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// let token = CancellationToken::new();
///
/// let discovered_iter = discover(&config.discovery, &resolved, &token)?;
/// let processed_iter = process_files(discovered_iter, &config.processing, &token);
///
/// let processed_files: Vec<_> = processed_iter.collect::<dircat::errors::Result<_>>()?;
/// println!("Processed {} files.", processed_files.len());
/// # Ok(())
/// # }
pub fn process_files<'a>(
    files: impl Iterator<Item = FileInfo> + Send + 'a,
    config: &'a ProcessingConfig,
    token: &'a CancellationToken,
) -> impl Iterator<Item = Result<FileInfo>> {
    // Bridge the sequential iterator to a parallel one for processing,
    // then collect the results into a vector to return a sequential iterator.
    let par_iter = files.par_bridge();
    process_and_filter_files_internal(par_iter, config, token)
        .collect::<Vec<_>>()
        .into_iter()
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
        config
            .processing
            .content_filters
            .push(Box::new(RemoveCommentsFilter));
        config
            .processing
            .content_filters
            .push(Box::new(RemoveEmptyLinesFilter));

        let processed: Vec<_> = process_and_filter_files_internal(
            vec![file_info].into_par_iter(),
            &config.processing,
            &token,
        )
        .collect::<Result<_>>()?;

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
        assert!(config.processing.content_filters.is_empty());

        let processed: Vec<_> = process_and_filter_files_internal(
            vec![file_info].into_par_iter(),
            &config.processing,
            &token,
        )
        .collect::<Result<_>>()?;

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
        config.processing.include_binary = true; // We must include it to process it
        config
            .processing
            .content_filters
            .push(Box::new(RemoveCommentsFilter));

        let processed: Vec<_> = process_and_filter_files_internal(
            vec![file_info].into_par_iter(),
            &config.processing,
            &token,
        )
        .collect::<Result<_>>()?;

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
