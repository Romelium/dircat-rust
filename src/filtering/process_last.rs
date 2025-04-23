// src/filtering/process_last.rs

use crate::config::Config;
use glob::Pattern;
use log::warn; // Import warn
use std::ffi::OsStr;
use std::path::Path;

/// Checks if a file path matches any of the `-z`/`--last` glob patterns.
/// Returns `(matches, index_of_matching_pattern)`.
/// Matches against the relative path.
pub(crate) fn check_process_last(
    // Changed to pub(crate)
    relative_path: &Path,
    _filename: Option<&OsStr>, // Filename no longer needed for matching
    config: &Config,
) -> (bool, Option<usize>) {
    if let Some(ref last_patterns) = config.process_last {
        for (index, pattern_str) in last_patterns.iter().enumerate() {
            match Pattern::new(pattern_str) {
                Ok(pattern) => {
                    // Match the glob pattern against the relative path
                    if pattern.matches_path(relative_path) {
                        return (true, Some(index));
                    }
                }
                Err(e) => {
                    // Log a warning for invalid patterns but continue checking others
                    warn!(
                        "Invalid glob pattern in --last argument: '{}'. Error: {}",
                        pattern_str, e
                    );
                }
            }
        }
    }
    (false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OutputDestination};
    use std::path::PathBuf;

    fn create_test_config(process_last: Option<Vec<&str>>) -> Config {
        Config {
            input_path: PathBuf::from("/base"), // Mock base path
            base_path_display: "/base".to_string(),
            input_is_file: false,
            process_last: process_last.map(|v| v.iter().map(|s| s.to_string()).collect()),
            // Other fields defaulted
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            path_regex: None,
            filename_regex: None,
            use_gitignore: true,
            include_binary: false, // <-- ADDED field
            skip_lockfiles: false, // <-- ADDED field
            remove_comments: false,
            remove_empty_lines: false,
            filename_only_header: false,
            line_numbers: false,
            backticks: false,
            output_destination: OutputDestination::Stdout,
            summary: false,
            counts: false,
            only_last: false,
            dry_run: false,
        }
    }

    #[test]
    fn test_no_last_patterns() {
        let config = create_test_config(None);
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (false, None)
        );
    }

    #[test]
    fn test_match_exact_filename_glob() {
        let config = create_test_config(Some(vec!["other.txt", "main.rs"]));
        let rel_path = Path::new("src/main.rs"); // Relative path includes directory
        let filename = rel_path.file_name();
        // "main.rs" glob does NOT match "src/main.rs"
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (false, None)
        );

        let rel_path_root = Path::new("main.rs"); // Relative path is just filename
        let filename_root = rel_path_root.file_name();
        // "main.rs" glob DOES match "main.rs"
        assert_eq!(
            check_process_last(rel_path_root, filename_root, &config),
            (true, Some(1))
        );
    }

    #[test]
    fn test_match_wildcard_filename_glob() {
        let config = create_test_config(Some(vec!["*.txt", "*.rs"]));
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        // "*.rs" glob DOES match "src/main.rs"
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (true, Some(1))
        );

        let rel_path_txt = Path::new("docs/README.txt");
        let filename_txt = rel_path_txt.file_name();
        // "*.txt" glob DOES match "docs/README.txt"
        assert_eq!(
            check_process_last(rel_path_txt, filename_txt, &config),
            (true, Some(0))
        );
    }

    #[test]
    fn test_match_relative_path_glob() {
        let config = create_test_config(Some(vec!["src/*.rs", "data/**/config.toml"]));
        let rel_path = Path::new("src/lib.rs");
        let filename = rel_path.file_name();
        // "src/*.rs" glob DOES match "src/lib.rs"
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (true, Some(0))
        );

        let rel_path_toml = Path::new("data/prod/config.toml");
        let filename_toml = rel_path_toml.file_name();
        // "data/**/config.toml" glob DOES match "data/prod/config.toml"
        assert_eq!(
            check_process_last(rel_path_toml, filename_toml, &config),
            (true, Some(1))
        );

        let rel_path_toml_deep = Path::new("data/dev/nested/config.toml");
        let filename_toml_deep = rel_path_toml_deep.file_name();
        // "data/**/config.toml" glob DOES match "data/dev/nested/config.toml"
        assert_eq!(
            check_process_last(rel_path_toml_deep, filename_toml_deep, &config),
            (true, Some(1))
        );
    }

    #[test]
    fn test_no_match_glob() {
        let config = create_test_config(Some(vec!["lib.rs", "*.toml"]));
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (false, None)
        );
    }

    #[test]
    fn test_invalid_glob_pattern() {
        // Should log a warning but not panic, and not match
        let config = create_test_config(Some(vec!["[invalid", "*.rs"])); // First pattern is invalid
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        // Should still match the second, valid pattern
        assert_eq!(
            check_process_last(rel_path, filename, &config),
            (true, Some(1))
        );

        let rel_path_other = Path::new("src/other.txt");
        let filename_other = rel_path_other.file_name();
        // Should not match anything
        assert_eq!(
            check_process_last(rel_path_other, filename_other, &config),
            (false, None)
        );
        // Check logs manually or using a test logger if needed to confirm warning
    }
}
