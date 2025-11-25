//! Discovers files based on configuration, applying filters in parallel.
use crate::cancellation::CancellationToken;
use crate::config::path_resolve::ResolvedInput;
use crate::config::DiscoveryConfig;
use crate::core_types::FileInfo;
use crate::errors::{Error, Result};
use crossbeam_channel::unbounded;
use ignore::WalkState;
use log::info;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

mod entry_processor;
mod walker;

use entry_processor::process_direntry;
use walker::build_walker;

/// Discovers files based on the provided configuration, applying filters.
///
/// This function walks the filesystem according to the rules in the `DiscoveryConfig`
/// (respecting .gitignore, filters, etc.) and returns two vectors of `FileInfo`
/// structs: one for normally processed files and one for files matching the
/// `--last` patterns. The content of the files is not read at this stage.
///
/// # Arguments
/// * `config` - The configuration for the discovery process.
/// * `resolved` - The `ResolvedInput` struct containing the resolved, absolute path information.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
///
/// # Returns
/// A `Result` containing a tuple of two vectors: `(normal_files, last_files)`.
/// The `last_files` vector is sorted according to the order of the `--last` patterns
/// and then alphabetically. The `normal_files` vector is explicitly **not sorted**
/// and its order is non-deterministic due to parallel processing.
///
/// # Errors
/// Returns an `Error` if the operation is interrupted or if building the file walker fails.
///
/// # Examples
///
/// ```
/// use dircat::config::{self, ConfigBuilder};
/// use dircat::{discover_files, CancellationToken};
/// use tempfile::tempdir;
/// use std::fs;
///
/// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// // 1. Set up a temporary directory with some files.
/// let temp = tempdir()?;
/// fs::write(temp.path().join("a.txt"), "A")?;
/// fs::write(temp.path().join("b.md"), "B")?;
///
/// // 2. Build a configuration to find the files.
/// let config = ConfigBuilder::new()
///     .input_path(temp.path().to_str().unwrap())
///     .process_last(vec!["*.md".to_string()])
///     .build()?;
///
/// // 3. Resolve the input path and create a cancellation token.
/// let resolved = config::resolve_input(&config.input_path, &None, None, &None, None)?;
/// let token = CancellationToken::new();
///
/// // 4. Discover the files.
/// let (normal_files, last_files) = discover_files(&config.discovery, &resolved, &token)?;
/// assert_eq!(normal_files.len(), 1);
/// assert_eq!(last_files.len(), 1);
/// # Ok(())
/// # }
/// ```
pub fn discover_files(
    config: &DiscoveryConfig,
    resolved: &ResolvedInput,
    token: &CancellationToken,
) -> Result<(Vec<FileInfo>, Vec<FileInfo>)> {
    info!("Starting file discovery in: {}", resolved.path.display());

    let mut normal_files = Vec::new();
    let mut last_files = Vec::<FileInfo>::new();

    // Check for stop signal before starting the walk
    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    let (walker, _temp_file_guard) = build_walker(config, resolved)?;
    let (tx, rx) = unbounded();

    // Clone the necessary data to move into the static closure.
    let config_clone = config.clone();
    let resolved_clone = resolved.clone();
    let token_clone = token.clone();

    // Shared atomic counter for the walker threads to track discovered files
    let file_counter = Arc::new(AtomicUsize::new(0));
    let file_counter_clone = file_counter.clone();

    walker.run(move || {
        // This factory closure is 'static and is called for each thread.
        // We clone the data again for each thread's closure.
        let tx = tx.clone();
        let token = token_clone.clone();
        let config = config_clone.clone();
        let resolved = resolved_clone.clone();
        let file_counter = file_counter_clone.clone();

        Box::new(move |entry_result| {
            if token.is_cancelled() {
                return WalkState::Quit;
            }

            // SECURITY: Check file count limit (fast check)
            if let Some(limit) = config.max_file_count {
                if file_counter.load(Ordering::Relaxed) >= limit {
                    return WalkState::Quit;
                }
            }

            match process_direntry(entry_result, &config, &resolved) {
                Ok(Some(file_info)) => {
                    // Increment counter and check limit again before sending
                    if let Some(limit) = config.max_file_count {
                        let current_count = file_counter.fetch_add(1, Ordering::Relaxed);
                        if current_count >= limit {
                            log::warn!("Safe Mode: File count limit ({}) reached.", limit);
                            return WalkState::Quit;
                        }
                    }

                    if tx.send(file_info).is_err() {
                        log::error!("Receiver dropped, quitting discovery walk.");
                        return WalkState::Quit;
                    }
                }
                Ok(None) => { /* Entry was filtered out, do nothing */ }
                Err(e) => {
                    // Log the warning from entry_processor but continue walking
                    log::warn!("{}", e);
                }
            }
            WalkState::Continue
        })
    });

    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    for file_info in rx {
        if file_info.is_process_last {
            last_files.push(file_info);
        } else if !config.only_last {
            normal_files.push(file_info);
        }
    }

    // Sort the "last" files first by the order of the matching -z pattern,
    // and then alphabetically by path to ensure deterministic output.
    // Using a tuple as a key sorts by the first element, then the second for ties.
    last_files.sort_by_key(|fi| (fi.process_last_order, fi.relative_path.clone()));

    info!(
        "Discovery complete. Normal files: {}, Last files: {}",
        normal_files.len(),
        last_files.len()
    );
    Ok((normal_files, last_files))
}
