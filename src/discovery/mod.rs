//! Discovers files based on configuration, applying filters in parallel.
use crate::cancellation::CancellationToken;
use crate::config::path_resolve::ResolvedInput;
use crate::config::DiscoveryConfig;
use crate::core_types::FileInfo;
use crate::errors::{Error, Result};
use crossbeam_channel::unbounded;
use ignore::WalkState;
use log::debug;

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
/// and then alphabetically. The `normal_files` vector is not sorted.
///
/// # Errors
/// Returns an `Error` if the operation is interrupted or if building the file walker fails.
///
/// # Examples
///
/// ```
/// use dircat::config::{ConfigBuilder, resolve_input};
/// use dircat::cancellation::CancellationToken;
/// use dircat::discover_files;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ConfigBuilder::new().input_path(".").build()?;
/// // In a real app, resolve_input would handle git clones etc.
/// let resolved = resolve_input(&config.input_path, &None, None, &None, None)?;
/// let token = CancellationToken::new();
///
/// let (normal_files, last_files) = discover_files(&config.discovery, &resolved, &token)?;
/// println!("Found {} normal files and {} 'last' files.", normal_files.len(), last_files.len());
/// # Ok(())
/// # }
/// ```
pub fn discover_files(
    config: &DiscoveryConfig,
    resolved: &ResolvedInput,
    token: &CancellationToken,
) -> Result<(Vec<FileInfo>, Vec<FileInfo>)> {
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

    walker.run(move || {
        // This factory closure is 'static and is called for each thread.
        // We clone the data again for each thread's closure.
        let tx = tx.clone();
        let token = token_clone.clone();
        let config = config_clone.clone();
        let resolved = resolved_clone.clone();

        Box::new(move |entry_result| {
            if token.is_cancelled() {
                return WalkState::Quit;
            }
            match process_direntry(entry_result, &config, &resolved) {
                Ok(Some(file_info)) => {
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

    debug!(
        "Discovery complete. Normal files: {}, Last files: {}",
        normal_files.len(),
        last_files.len()
    );
    Ok((normal_files, last_files))
}
