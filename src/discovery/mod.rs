use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::Error;
use anyhow::Result;
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod entry_processor;
mod walker;

use entry_processor::process_direntry;
use walker::build_walker;

/// Discovers files based on the provided configuration, applying filters.
/// Returns a tuple of two vectors: (normal_files, last_files).
pub fn discover_files(
    config: &Config,
    stop_signal: Arc<AtomicBool>,
) -> Result<(Vec<FileInfo>, Vec<FileInfo>)> {
    let mut normal_files = Vec::new();
    let mut last_files = Vec::<FileInfo>::new();

    debug!(
        "Starting file discovery in: {}",
        config.input_path.display()
    );

    let (walker, _temp_file_guard) = build_walker(config)?;

    for entry_result in walker {
        // Check for Ctrl+C signal
        if !stop_signal.load(Ordering::Relaxed) {
            // Check if FALSE
            // Persistent Debug Log: Add log here
            log::error!("Discovery loop interrupted by stop_signal!");
            return Err(Error::Interrupted.into());
        }

        match process_direntry(entry_result, config) {
            Ok(Some(file_info)) => {
                if file_info.is_process_last {
                    last_files.push(file_info);
                } else if !config.only_last {
                    // Add to normal files only if not in "only_last" mode
                    normal_files.push(file_info);
                }
            }
            Ok(None) => { /* Entry was filtered out, do nothing */ }
            Err(e) => {
                // Log the warning from entry_processor but continue walking
                log::warn!("{}", e);
            }
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
