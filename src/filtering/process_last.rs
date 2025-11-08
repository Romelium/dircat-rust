// src/filtering/process_last.rs

use crate::discovery::DiscoveryOptions;
use glob::Pattern;
use log::warn; // Import warn
use std::ffi::OsStr;
use std::path::Path;

/// Checks if a file path matches any of the `-z`/`--last` glob patterns.
/// Returns `(matches, index_of_matching_pattern)`.
/// Matches against the relative path.
///
/// This function is used to determine if a file should be part of the "process last"
/// group. The returned index corresponds to the order in which the glob patterns
/// were provided on the command line, which is used for sorting these files.
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
/// use dircat::filtering::check_process_last;
/// use std::path::Path;
///
/// let config = ConfigBuilder::new()
///     .process_last(vec!["README.md".to_string(), "*.toml".to_string()])
///     .build().unwrap();
/// let opts = dircat::discovery::DiscoveryOptions::from(&config);
///
/// let path1 = Path::new("README.md");
/// assert_eq!(check_process_last(path1, path1.file_name(), &opts), (true, Some(0)));
///
/// let path2 = Path::new("config/app.toml");
/// assert_eq!(check_process_last(path2, path2.file_name(), &opts), (true, Some(1)));
///
/// let path3 = Path::new("src/main.rs");
/// assert_eq!(check_process_last(path3, path3.file_name(), &opts), (false, None));
/// ```
pub fn check_process_last(
    relative_path: &Path,
    _filename: Option<&OsStr>, // Filename no longer needed for matching
    opts: &DiscoveryOptions,
) -> (bool, Option<usize>) {
    if let Some(ref last_patterns) = opts.process_last {
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
    use crate::config::ConfigBuilder;
    use std::path::Path;

    fn create_test_opts(process_last: Option<Vec<&str>>) -> DiscoveryOptions<'static> {
        let mut builder = ConfigBuilder::new().input_path("/base");
        if let Some(p) = process_last {
            builder = builder.process_last(p.iter().map(|s| s.to_string()).collect());
        }
        let config = Box::leak(Box::new(builder.build().unwrap()));
        DiscoveryOptions::from(&*config)
    }

    #[test]
    fn test_no_last_patterns() {
        let opts = create_test_opts(None);
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        assert_eq!(check_process_last(rel_path, filename, &opts), (false, None));
    }

    #[test]
    fn test_match_exact_filename_glob() {
        let opts = create_test_opts(Some(vec!["other.txt", "main.rs"]));
        let rel_path = Path::new("src/main.rs"); // Relative path includes directory
        let filename = rel_path.file_name();
        // "main.rs" glob does NOT match "src/main.rs"
        assert_eq!(check_process_last(rel_path, filename, &opts), (false, None));

        let rel_path_root = Path::new("main.rs"); // Relative path is just filename
        let filename_root = rel_path_root.file_name();
        // "main.rs" glob DOES match "main.rs"
        assert_eq!(
            check_process_last(rel_path_root, filename_root, &opts),
            (true, Some(1))
        );
    }

    #[test]
    fn test_match_wildcard_filename_glob() {
        let opts = create_test_opts(Some(vec!["*.txt", "*.rs"]));
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        // "*.rs" glob DOES match "src/main.rs"
        assert_eq!(
            check_process_last(rel_path, filename, &opts),
            (true, Some(1))
        );

        let rel_path_txt = Path::new("docs/README.txt");
        let filename_txt = rel_path_txt.file_name();
        // "*.txt" glob DOES match "docs/README.txt"
        assert_eq!(
            check_process_last(rel_path_txt, filename_txt, &opts),
            (true, Some(0))
        );
    }

    #[test]
    fn test_match_relative_path_glob() {
        let opts = create_test_opts(Some(vec!["src/*.rs", "data/**/config.toml"]));
        let rel_path = Path::new("src/lib.rs");
        let filename = rel_path.file_name();
        // "src/*.rs" glob DOES match "src/lib.rs"
        assert_eq!(
            check_process_last(rel_path, filename, &opts),
            (true, Some(0))
        );

        let rel_path_toml = Path::new("data/prod/config.toml");
        let filename_toml = rel_path_toml.file_name();
        // "data/**/config.toml" glob DOES match "data/prod/config.toml"
        assert_eq!(
            check_process_last(rel_path_toml, filename_toml, &opts),
            (true, Some(1))
        );

        let rel_path_toml_deep = Path::new("data/dev/nested/config.toml");
        let filename_toml_deep = rel_path_toml_deep.file_name();
        // "data/**/config.toml" glob DOES match "data/dev/nested/config.toml"
        assert_eq!(
            check_process_last(rel_path_toml_deep, filename_toml_deep, &opts),
            (true, Some(1))
        );
    }

    #[test]
    fn test_no_match_glob() {
        let opts = create_test_opts(Some(vec!["lib.rs", "*.toml"]));
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        assert_eq!(check_process_last(rel_path, filename, &opts), (false, None));
    }

    #[test]
    fn test_invalid_glob_pattern() {
        // Should log a warning but not panic, and not match
        let opts = create_test_opts(Some(vec!["[invalid", "*.rs"])); // First pattern is invalid
        let rel_path = Path::new("src/main.rs");
        let filename = rel_path.file_name();
        // Should still match the second, valid pattern
        assert_eq!(
            check_process_last(rel_path, filename, &opts),
            (true, Some(1))
        );

        let rel_path_other = Path::new("src/other.txt");
        let filename_other = rel_path_other.file_name();
        // Should not match anything
        assert_eq!(
            check_process_last(rel_path_other, filename_other, &opts),
            (false, None)
        );
        // Check logs manually or using a test logger if needed to confirm warning
    }
}
