// src/discovery/entry_processor.rs

use crate::config::Config;
use crate::core_types::FileInfo;
use crate::errors::AppError; // Removed unused io_error_with_path
use crate::filtering::{
    check_process_last, is_file_type, passes_extension_filters, passes_size_filter,
};
use anyhow::Result;
use ignore::DirEntry;
use log::{debug, trace, warn}; // Import trace and warn
use std::path::{Path, PathBuf}; // Added PathBuf import
use tracing::instrument; // Keep tracing import

/// Processes a single directory entry from the walk.
///
/// Performs filtering based on type, size, extensions, gitignore rules (handled by walker),
/// custom ignore patterns (handled by walker), path regex, and filename regex.
///
/// Returns `Ok(Some(FileInfo))` if the entry is a file that passes all filters.
/// Returns `Ok(None)` if the entry is filtered out or is not a regular file.
/// Returns `Err(AppError)` for critical errors (like permission issues accessing metadata).
// Changed pub(super) to pub(crate) for visibility from src/discovery/mod.rs
pub(crate) fn process_direntry(
    entry_result: Result<DirEntry, ignore::Error>,
    config: &Config,
) -> Result<Option<FileInfo>, AppError> {
    // --- 1. Handle Walker Errors ---
    let entry = match entry_result {
        Ok(entry) => entry,
        Err(ignore_error) => {
            // Log walker errors (like permission denied during traversal) but try to continue.
            // Convert ignore::Error to a more general AppError or log directly.
            // Using warn! as these are often non-fatal for the overall process.
            warn!("Walker error: {}", ignore_error);
            return Ok(None); // Skip this entry
        }
    };

    let absolute_path = entry.path().to_path_buf();
    trace!("Processing entry: {}", absolute_path.display());

    // --- 2. Calculate Relative Path ---
    // Handle file input case separately
    let relative_path = if config.input_is_file {
        // If input was a file, the relative path is just its filename.
        // Use the filename from the absolute path of the entry.
        absolute_path
            .file_name()
            .map(PathBuf::from) // Convert OsStr to PathBuf
            .unwrap_or_else(|| {
                // Fallback: Should not happen if input_is_file is true, but handle defensively.
                warn!(
                    "Could not get filename for file input: {}",
                    absolute_path.display()
                );
                absolute_path.clone() // Use absolute path as a last resort
            })
    } else {
        // If input was a directory, strip the base input path prefix.
        absolute_path
            .strip_prefix(&config.input_path)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|err| {
                // Fallback if stripping fails (e.g., path outside base somehow?)
                warn!(
                    "Failed to strip prefix '{}' from '{}': {}. Using absolute path.",
                    config.input_path.display(),
                    absolute_path.display(),
                    err
                );
                absolute_path.clone() // Use absolute path as fallback
            })
    };
    trace!("Calculated relative path: {}", relative_path.display());

    // --- 3. Get Metadata ---
    // Use entry.metadata() which might be cached by the walker.
    let metadata = match entry.metadata() {
        Ok(md) => md,
        Err(e) => {
            // FIX: Cannot convert ignore::Error to std::io::Error directly.
            // Log the ignore::Error directly and skip the entry.
            warn!(
                "Skipping entry '{}' due to metadata error: {}",
                absolute_path.display(),
                e // Log the original ignore::Error
            );
            return Ok(None); // Skip files we can't get metadata for
        }
    };

    // --- 4. Filter by File Type ---
    if !is_file_type(&metadata) {
        trace!("Skipping non-file entry: {}", absolute_path.display());
        return Ok(None);
    }
    trace!("Entry is a file: {}", absolute_path.display());

    // --- 5. Filter by Size ---
    if !passes_size_filter(&metadata, config) {
        debug!(
            "Skipping file due to size constraint: {} (Size: {} bytes)",
            absolute_path.display(),
            metadata.len()
        );
        return Ok(None);
    }
    trace!("File passed size filter: {}", absolute_path.display());

    // --- 6. Filter by Extension ---
    if !passes_extension_filters(&absolute_path, config) {
        debug!(
            "Skipping file due to extension filter: {}",
            absolute_path.display()
        );
        return Ok(None);
    }
    trace!("File passed extension filter: {}", absolute_path.display());

    // --- 7. Filter by Regex (Path and Filename) ---
    // Pass both absolute and relative paths for correct matching.
    if !passes_regex_filters(&absolute_path, &relative_path, config)? {
        debug!(
            "Skipping file due to regex filter: {}",
            absolute_path.display()
        );
        return Ok(None);
    }
    trace!("File passed regex filters: {}", absolute_path.display());

    // --- 8. Check if it matches a "process last" pattern ---
    let (is_last, last_order) =
        check_process_last(&relative_path, absolute_path.file_name(), config);
    if is_last {
        trace!(
            "File marked as 'process last' (order {:?}): {}",
            last_order,
            relative_path.display()
        );
    }

    // --- 9. Construct FileInfo ---
    // If we reach here, the file passed all filters.
    let file_info = FileInfo {
        absolute_path,
        relative_path,
        size: metadata.len(),
        processed_content: None, // Content is read later in the processing stage
        counts: None,            // Counts are calculated later
        is_process_last: is_last,
        process_last_order: last_order,
    };

    debug!(
        "Entry passed all filters: {}",
        file_info.relative_path.display()
    );
    Ok(Some(file_info))
}

/// Checks if a file passes the path and filename regex filters.
/// Path regex is matched against the relative path.
/// Filename regex is matched against the filename component.
// Removed pub(crate) - function is only used within this module
#[instrument(level = "debug", skip(config), fields(relative_path = %relative_path.display(), filename = ?path.file_name()))]
fn passes_regex_filters(
    path: &Path,          // Absolute path for filename extraction
    relative_path: &Path, // Relative path for path regex matching
    config: &Config,
) -> Result<bool, AppError> {
    // Check path regex filter against the relative path
    if let Some(path_regex_vec) = &config.path_regex {
        // Normalize relative path separators for consistent matching
        let relative_path_str = relative_path.to_string_lossy().replace('\\', "/");
        // Check if *any* regex in the vector matches
        let matches = path_regex_vec
            .iter()
            .any(|re| re.is_match(&relative_path_str));
        // FIX: Remove .as_ref()
        debug!(
            "Checking path regex vector against relative path: regexes={:?}, path={}",
            path_regex_vec,
            relative_path_str, // Pass String directly
        );
        if !matches {
            debug!("Path regex vector did not match relative path");
            return Ok(false);
        }
        debug!("Path regex vector matched relative path");
    }

    // Check filename regex filter against the filename component of the absolute path
    if let Some(filename_regex_vec) = &config.filename_regex {
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();
            // Check if *any* regex in the vector matches
            let matches = filename_regex_vec
                .iter()
                .any(|re| re.is_match(&filename_str));
            // FIX: Remove .as_ref()
            debug!(
                "Checking filename regex vector: regexes={:?}, filename={}",
                filename_regex_vec,
                filename_str, // Pass Cow<'_, str> directly
            );
            if !matches {
                debug!("Filename regex vector did not match");
                return Ok(false);
            }
            debug!("Filename regex vector matched");
        } else {
            // If there's no filename (e.g., path is "." or ".."), it cannot match a filename regex.
            debug!("Path has no filename component, failing filename regex match");
            return Ok(false);
        }
    }

    // If we reached here, all applicable regex filters passed
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OutputDestination};
    use regex::Regex;
    use std::fs::{self, File};
    use std::path::PathBuf;
    use tempfile::tempdir;

    // Helper to create a Config with specific regex filters
    fn create_config_with_regex(
        path_patterns: Option<Vec<&str>>,
        filename_patterns: Option<Vec<&str>>,
    ) -> Config {
        let path_regex = path_patterns.map(|patterns| {
            patterns
                .iter()
                .map(|p| Regex::new(p).unwrap())
                .collect::<Vec<_>>()
        });
        let filename_regex = filename_patterns.map(|patterns| {
            patterns
                .iter()
                .map(|p| Regex::new(p).unwrap())
                .collect::<Vec<_>>()
        });

        Config {
            input_path: PathBuf::from("."), // Mock path
            base_path_display: ".".to_string(),
            input_is_file: false, // Default to directory input for these tests
            path_regex,
            filename_regex,
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            use_gitignore: true,
            remove_comments: false,
            remove_empty_lines: false,
            filename_only_header: false,
            line_numbers: false,
            backticks: false,
            output_destination: OutputDestination::Stdout,
            summary: false,
            counts: false,
            process_last: None,
            only_last: false,
            dry_run: false,
        }
    }

    // Helper to create paths for testing
    fn create_paths(base: &Path, relative: &str) -> (PathBuf, PathBuf) {
        (base.join(relative), PathBuf::from(relative))
    }

    #[test]
    fn test_regex_no_filters() -> Result<(), AppError> {
        let dir = tempdir().unwrap();
        let (abs_path, rel_path) = create_paths(dir.path(), "test_file.txt");
        File::create(&abs_path).unwrap();
        let config = create_config_with_regex(None, None);
        assert!(passes_regex_filters(&abs_path, &rel_path, &config)?);
        Ok(())
    }

    #[test]
    fn test_regex_path_match() -> Result<(), AppError> {
        let dir = tempdir().unwrap();
        let (abs_path1, rel_path1) = create_paths(dir.path(), "subdir/match_file.txt");
        let (abs_path2, rel_path2) = create_paths(dir.path(), "no_match_file.txt");
        fs::create_dir_all(abs_path1.parent().unwrap()).unwrap();
        File::create(&abs_path1).unwrap();
        File::create(&abs_path2).unwrap();

        // Regex that should match rel_path1 (starts with "subdir/")
        let config = create_config_with_regex(Some(vec!["^subdir/"]), None);

        assert!(passes_regex_filters(&abs_path1, &rel_path1, &config)?);
        assert!(!passes_regex_filters(&abs_path2, &rel_path2, &config)?);
        Ok(())
    }

    #[test]
    fn test_regex_path_match_windows_style_relative() -> Result<(), AppError> {
        // Simulate a windows-style relative path string for the regex
        let dir = tempdir().unwrap();
        // Relative path uses backslashes, but gets normalized
        let (abs_path, rel_path) = create_paths(dir.path(), "subdir\\match_file.txt");
        fs::create_dir_all(abs_path.parent().unwrap()).unwrap();
        File::create(&abs_path).unwrap();

        // Regex uses forward slashes, should match normalized relative path
        let config_fwd = create_config_with_regex(Some(vec!["^subdir/match"]), None);
        // Regex uses backslashes, should NOT match normalized relative path
        let config_bwd = create_config_with_regex(Some(vec![r"^subdir\\match"]), None);

        assert!(passes_regex_filters(&abs_path, &rel_path, &config_fwd)?);
        assert!(!passes_regex_filters(&abs_path, &rel_path, &config_bwd)?);
        Ok(())
    }

    #[test]
    fn test_regex_filename_match() -> Result<(), AppError> {
        let dir = tempdir().unwrap();
        let (abs_path1, rel_path1) = create_paths(dir.path(), "match_this.log");
        let (abs_path2, rel_path2) = create_paths(dir.path(), "ignore_this.txt");
        File::create(&abs_path1).unwrap();
        File::create(&abs_path2).unwrap();

        // Regex that should match path1's filename
        let config = create_config_with_regex(None, Some(vec![r"^match_.*\.log$"]));

        assert!(passes_regex_filters(&abs_path1, &rel_path1, &config)?);
        assert!(!passes_regex_filters(&abs_path2, &rel_path2, &config)?);
        Ok(())
    }

    #[test]
    fn test_regex_path_and_filename_match() -> Result<(), AppError> {
        let dir = tempdir().unwrap();
        let (abs_path1, rel_path1) = create_paths(dir.path(), "target_dir/target_file.rs"); // Match both
        let (abs_path2, rel_path2) = create_paths(dir.path(), "target_dir/other_file.rs"); // Match path, fail filename
        let (abs_path3, rel_path3) = create_paths(dir.path(), "other_dir/target_file.rs"); // Fail path, match filename
        let (abs_path4, rel_path4) = create_paths(dir.path(), "other_dir/another_file.txt"); // Fail both
        fs::create_dir_all(abs_path1.parent().unwrap()).unwrap();
        fs::create_dir_all(abs_path3.parent().unwrap()).unwrap();
        File::create(&abs_path1).unwrap();
        File::create(&abs_path2).unwrap();
        File::create(&abs_path3).unwrap();
        File::create(&abs_path4).unwrap();

        // Regexes that should match path1
        let config = create_config_with_regex(
            Some(vec!["^target_dir/"]),       // Matches relative path
            Some(vec![r"^target_file\.rs$"]), // Matches filename
        );

        assert!(passes_regex_filters(&abs_path1, &rel_path1, &config)?); // Both match
        assert!(!passes_regex_filters(&abs_path2, &rel_path2, &config)?); // Filename fails
        assert!(!passes_regex_filters(&abs_path3, &rel_path3, &config)?); // Path fails
        assert!(!passes_regex_filters(&abs_path4, &rel_path4, &config)?); // Both fail
        Ok(())
    }

    #[test]
    fn test_regex_no_filename() -> Result<(), AppError> {
        // Test behavior when the path itself has no filename component (e.g., ".")
        let current_dir_abs = PathBuf::from("."); // Represents current dir
        let current_dir_rel = PathBuf::from(".");

        let config_filename = create_config_with_regex(None, Some(vec!["anything"])); // Has filename regex
                                                                                      // Should fail because "." has no filename component
        assert!(!passes_regex_filters(
            &current_dir_abs,
            &current_dir_rel,
            &config_filename
        )?);

        let config_path = create_config_with_regex(Some(vec![r"^\.$"]), None); // Match path "."
                                                                               // Should pass because path regex matches relative path "."
        assert!(passes_regex_filters(
            &current_dir_abs,
            &current_dir_rel,
            &config_path
        )?);

        Ok(())
    }
}
