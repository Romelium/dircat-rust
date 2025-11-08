use crate::cancellation::CancellationToken;
use crate::config::path_resolve::ResolvedInput;
use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::{Error, Result};
use log::debug;

mod entry_processor;
mod walker;

use entry_processor::process_direntry;
use walker::build_walker;

/// A struct holding only the configuration options relevant to file discovery.
pub struct DiscoveryOptions<'a> {
    pub recursive: bool,
    pub extensions: &'a Option<Vec<String>>,
    pub exclude_extensions: &'a Option<Vec<String>>,
    pub ignore_patterns: &'a Option<Vec<String>>,
    pub exclude_path_regex: &'a Option<Vec<regex::Regex>>,
    pub path_regex: &'a Option<Vec<regex::Regex>>,
    pub filename_regex: &'a Option<Vec<regex::Regex>>,
    pub use_gitignore: bool,
    pub skip_lockfiles: bool,
    pub max_size: Option<u128>,
    pub process_last: &'a Option<Vec<String>>,
    pub only_last: bool,
}

impl<'a> From<&'a Config> for DiscoveryOptions<'a> {
    fn from(config: &'a Config) -> Self {
        Self {
            recursive: config.recursive,
            extensions: &config.extensions,
            exclude_extensions: &config.exclude_extensions,
            ignore_patterns: &config.ignore_patterns,
            exclude_path_regex: &config.exclude_path_regex,
            path_regex: &config.path_regex,
            filename_regex: &config.filename_regex,
            use_gitignore: config.use_gitignore,
            skip_lockfiles: config.skip_lockfiles,
            max_size: config.max_size,
            process_last: &config.process_last,
            only_last: config.only_last,
        }
    }
}

/// Discovers files based on the provided configuration, applying filters.
/// Returns a tuple of two vectors: (normal_files, last_files).
pub(crate) fn discover_files_internal(
    opts: &DiscoveryOptions,
    resolved: &ResolvedInput,
    token: &CancellationToken,
) -> Result<(Vec<FileInfo>, Vec<FileInfo>)> {
    let mut normal_files = Vec::new();
    let mut last_files = Vec::<FileInfo>::new();

    // Check for stop signal before starting the walk
    if token.is_cancelled() {
        return Err(Error::Interrupted);
    }

    let (walker, _temp_file_guard) = build_walker(opts, resolved)?;

    for entry_result in walker {
        // Check for Ctrl+C signal
        if token.is_cancelled() {
            log::error!("Discovery loop interrupted by token!");
            return Err(Error::Interrupted);
        }

        match process_direntry(entry_result, opts, resolved) {
            Ok(Some(file_info)) => {
                if file_info.is_process_last {
                    last_files.push(file_info);
                } else if !opts.only_last {
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

/// Discovers files using a specific set of `DiscoveryOptions`.
///
/// This is the advanced, granular entry point for the discovery stage. It allows for
/// creating `DiscoveryOptions` separately and passing them in, decoupling the discovery
/// logic from the main `Config` struct.
///
/// # Arguments
/// * `opts` - The discovery-specific options.
/// * `resolved` - The `ResolvedInput` struct containing the resolved, absolute path information.
/// * `token` - A `CancellationToken` that can be used to gracefully interrupt the process.
pub fn discover_with_options(
    opts: &DiscoveryOptions,
    resolved: &ResolvedInput,
    token: &CancellationToken,
) -> Result<(Vec<FileInfo>, Vec<FileInfo>)> {
    discover_files_internal(opts, resolved, token)
}
