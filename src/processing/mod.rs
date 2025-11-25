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
use log::{debug, trace};
use rayon::prelude::*;

use crate::core_types::FileContent;
pub mod counter;
pub mod filters;
pub use counter::calculate_counts;
use filters::ContentFilter;
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};

// Global memory usage tracker (in bytes) to prevent OOM
static GLOBAL_MEM_USAGE: AtomicUsize = AtomicUsize::new(0);
const MAX_GLOBAL_MEM_USAGE: usize = 2 * 1024 * 1024 * 1024; // 2GB Limit

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
/// This function is a specialized, low-level part of the processing pipeline that is
/// completely decoupled from filesystem I/O. It is ideal for scenarios where file
/// content is sourced from memory, a database, or a network stream, rather than
/// being read from local disk. It takes an iterator of `FileContent` structs and
/// performs the same content transformation and filtering logic as a normal run.
///
/// For most use cases, it is simpler to use the main `dircat::execute` function.
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
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let files_content = vec![FileContent {
///     relative_path: "a.rs".into(),
///     content: b"fn main() {}".to_vec(),
///     ..Default::default()
/// }];
/// let opts = ProcessingOptions { include_binary: false, counts: false, content_filters: &[] };
/// let token = CancellationToken::new();
/// let processed_files: Vec<_> = process_content(files_content.into_iter(), opts, &token).collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(processed_files.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn process_content<'a>(
    files_content: impl Iterator<Item = FileContent> + Send + 'a,
    opts: ProcessingOptions<'a>,
    token: &'a CancellationToken,
) -> impl Iterator<Item = Result<FileInfo>> {
    files_content
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
        .collect::<Vec<_>>()
        .into_iter()
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
    // Root path needed for LFI check in safe mode
    root_path: Option<&'a std::path::Path>,
) -> impl ParallelIterator<Item = Result<FileInfo>> + 'a {
    files.filter_map(move |mut file_info| {
        // The closure captures `config` and `token` which have lifetime 'a
        // Check for cancellation signal. If cancelled, return an error for this item
        // and subsequent calls will be filtered out by the was_cancelled flag.
        if token.is_cancelled() {
            return Some(Err(Error::Interrupted));
        }

        trace!("Reading content: {}", file_info.absolute_path.display());

        // --- 1. Read File Content (Optimized for Security) ---
        // Instead of reading the whole file immediately, read the head first to check for binary.
        let mut file = match File::open(&file_info.absolute_path) {
            Ok(f) => f,
            Err(e) => {
                return Some(Err(io_error_with_path(e, &file_info.absolute_path)));
            }
        };

        // --- SECURITY CHECKS (Moved after open to prevent TOCTOU) ---
        if let Some(sec) = &config.security {
            // Get metadata from the OPEN file handle
            let metadata = match file.metadata() {
                Ok(m) => m,
                Err(e) => return Some(Err(io_error_with_path(e, &file_info.absolute_path))),
            };

            // 1. DoS Protection: Check file size
            if metadata.len() > sec.max_file_size {
                log::warn!(
                    "Safe Mode: Skipping '{}' (size {} > limit {})",
                    file_info.relative_path.display(),
                    metadata.len(),
                    sec.max_file_size
                );
                return None;
            }

            // 2. LFI Protection: Verify file is inside the sandbox
            if let Some(root) = root_path {
                if let Err(e) =
                    sec.validate_file_access(&file_info.absolute_path, root, Some(&file))
                {
                    log::error!("{}", e);
                    return None;
                }
            }
        }

        // Read first 1KB for detection
        let mut head_buffer = [0u8; 1024];
        let head_bytes_read = match file.read(&mut head_buffer) {
            Ok(n) => n,
            Err(e) => {
                return Some(Err(io_error_with_path(e, &file_info.absolute_path)));
            }
        };
        debug!(
            "Read {} bytes from head of '{}'",
            head_bytes_read,
            file_info.relative_path.display()
        );

        // --- 2. Perform Binary Check ---
        let is_binary = !is_likely_text_from_buffer(&head_buffer[..head_bytes_read]);
        file_info.is_binary = is_binary;

        // --- 3. Filter Based on Binary Check ---
        if is_binary && !config.include_binary {
            trace!(
                "Skipping binary file: {}",
                file_info.relative_path.display()
            );
            return None; // Filter out this file by returning None
        }

        // SECURITY: Memory Exhaustion Check
        let size = file_info.size as usize;
        let current_usage = GLOBAL_MEM_USAGE.load(Ordering::Relaxed);
        if current_usage + size > MAX_GLOBAL_MEM_USAGE {
            log::error!(
                "OOM Protection: Skipping '{}' (Global memory limit exceeded)",
                file_info.relative_path.display()
            );
            return Some(Err(Error::Generic(anyhow::anyhow!(
                "Global memory limit exceeded"
            ))));
        }
        GLOBAL_MEM_USAGE.fetch_add(size, Ordering::Relaxed);

        // Read the rest of the file
        let mut content_bytes = Vec::with_capacity(file_info.size as usize);
        content_bytes.extend_from_slice(&head_buffer[..head_bytes_read]);
        let read_result = file.read_to_end(&mut content_bytes);

        // Release memory quota
        GLOBAL_MEM_USAGE.fetch_sub(size, Ordering::Relaxed);

        if let Err(e) = read_result {
            return Some(Err(io_error_with_path(e, &file_info.absolute_path)));
        }
        trace!(
            "Full content read for '{}': {} bytes",
            file_info.relative_path.display(),
            content_bytes.len()
        );

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
            trace!(
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
                let before_len = processed_content.len();
                processed_content = filter.apply(&processed_content);
                trace!(
                    "Applied filter '{}' to {} (len: {} -> {})",
                    filter.name(),
                    file_info.relative_path.display(),
                    before_len,
                    processed_content.len()
                );
            }
        } else {
            trace!(
                "Skipping content filters for binary file {}",
                file_info.relative_path.display()
            );
        }

        // Store the final processed content
        file_info.processed_content = Some(processed_content);

        Some(Ok(file_info))
    })
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
/// # use dircat::{config, discover, process_files};
/// # use std::fs;
/// # use tempfile::tempdir;
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// # let temp = tempdir()?;
/// # fs::write(temp.path().join("a.rs"), "A")?;
/// # fs::write(temp.path().join("c.txt"), "C")?;
/// # let config = ConfigBuilder::new().input_path(temp.path().to_str().unwrap()).build()?;
/// # let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// # let token = CancellationToken::new();
/// let discovered_iter = discover(&config.discovery, &resolved, &token)?;
/// let processed_iter = process_files(discovered_iter, &config.processing, &token);
///
/// // The order is not guaranteed, so we must collect and sort.
/// let mut processed_files: Vec<_> = processed_iter.collect::<Result<Vec<_>>>()?;
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
/// let discovered_iter = discover(&config.discovery, &resolved, &token)?;
/// let processed_iter = process_files(discovered_iter, &config.processing, &token);
///
/// let processed_files: Vec<_> = processed_iter.collect::<dircat::errors::Result<_>>()?;
/// assert_eq!(processed_files.len(), 1);
/// assert_eq!(processed_files[0].processed_content.as_deref(), Some("fn main(){}"));
/// # Ok(())
/// # }
/// ```
pub fn process_files<'a>(
    files: impl Iterator<Item = FileInfo> + Send + 'a,
    config: &'a ProcessingConfig,
    token: &'a CancellationToken,
    root_path: Option<&'a std::path::Path>,
) -> impl Iterator<Item = Result<FileInfo>> {
    // Bridge the sequential iterator to a parallel one for processing,
    // then collect the results into a vector to return a sequential iterator.
    process_and_filter_files_internal(files.par_bridge(), config, token, root_path)
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
            None,
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
            None,
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
            None,
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

    #[test]
    fn test_global_memory_limit_enforcement() {
        // 1. Setup
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("large_file.txt");
        let content = vec![b'a'; 1024 * 1024]; // 1MB file
        fs::write(&file_path, &content).unwrap();

        let file_info = FileInfo {
            absolute_path: file_path,
            relative_path: PathBuf::from("large_file.txt"),
            size: content.len() as u64,
            ..Default::default()
        };

        let config = crate::config::ProcessingConfig {
            include_binary: false,
            counts: false,
            content_filters: vec![],
            security: None,
        };
        let token = CancellationToken::new();

        // 2. Artificially inflate global memory usage to just below the limit
        // MAX is 2GB. Let's set it to MAX - 500KB.
        // The file is 1MB, so it should fail.
        let previous_usage =
            GLOBAL_MEM_USAGE.swap(MAX_GLOBAL_MEM_USAGE - (500 * 1024), Ordering::SeqCst);

        // 3. Attempt processing
        let result: Vec<_> = process_and_filter_files_internal(
            vec![file_info].into_par_iter(),
            &config,
            &token,
            None,
        )
        .collect();

        // 4. Reset global memory for other tests!
        GLOBAL_MEM_USAGE.store(previous_usage, Ordering::SeqCst);

        // 5. Assert failure
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        assert!(result[0]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("Global memory limit exceeded"));
    }

    #[test]
    fn test_global_memory_limit_allows_fitting_files() {
        // 1. Setup
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("small_file.txt");
        let content = vec![b'a'; 100]; // 100 bytes
        fs::write(&file_path, &content).unwrap();

        let file_info = FileInfo {
            absolute_path: file_path,
            relative_path: PathBuf::from("small_file.txt"),
            size: content.len() as u64,
            ..Default::default()
        };

        let config = crate::config::ProcessingConfig {
            include_binary: false,
            counts: false,
            content_filters: vec![],
            security: None,
        };
        let token = CancellationToken::new();

        // 2. Ensure memory usage is low
        let previous_usage = GLOBAL_MEM_USAGE.swap(0, Ordering::SeqCst);

        // 3. Attempt processing
        let result: Vec<_> = process_and_filter_files_internal(
            vec![file_info].into_par_iter(),
            &config,
            &token,
            None,
        )
        .collect();

        // 4. Reset
        GLOBAL_MEM_USAGE.store(previous_usage, Ordering::SeqCst);

        // 5. Assert success
        assert_eq!(result.len(), 1);
        assert!(result[0].is_ok());
    }
}
