// src/filtering/process_last.rs

use crate::config::DiscoveryConfig;
use glob::Pattern;
use log::warn; // Import warn
use std::path::Path;

/// Checks if a file's relative path matches any of the `-z`/`--last` glob patterns.
///
/// This function is used to determine if a file should be part of the "process last"
/// group. It matches the provided glob patterns against the file's relative path.
/// The returned index corresponds to the order in which the glob patterns
/// were provided on the command line, which is used for sorting these files.
///
/// # Returns
/// A tuple `(bool, Option<usize>)` where the `bool` is `true` if a match was found,
/// and the `Option<usize>` contains the index of the first matching pattern.
///
/// # Examples
///
/// ```
/// use dircat::config::DiscoveryConfig;
/// use dircat::filtering::check_process_last;
/// use std::path::Path;
///
/// let mut config = DiscoveryConfig::default_for_test();
/// config.process_last = Some(vec!["README.md".to_string(), "*.toml".to_string()]);
///
/// let path1 = Path::new("README.md");
/// assert_eq!(check_process_last(path1, &config), (true, Some(0)));
///
/// let path2 = Path::new("config/app.toml");
/// assert_eq!(check_process_last(path2, &config), (true, Some(1)));
///
/// let path3 = Path::new("src/main.rs");
/// assert_eq!(check_process_last(path3, &config), (false, None));
/// ```
pub fn check_process_last(relative_path: &Path, config: &DiscoveryConfig) -> (bool, Option<usize>) {
    if let Some(ref last_patterns) = config.process_last {
        for (index, pattern_str) in last_patterns.iter().enumerate() {
            // Check if the pattern should be treated as recursive (mimicking gitignore behavior).
            // If a pattern has no path separators (e.g. "*.rs"), gitignore applies it recursively.
            // Standard glob does not, so we manually check `**/pattern` as well in that case.
            let is_recursive_candidate = !pattern_str.contains('/') && !pattern_str.contains('\\');

            match Pattern::new(pattern_str) {
                Ok(pattern) => {
                    // 1. Try matching the pattern as-is (matches root files or explicit paths)
                    if pattern.matches_path(relative_path) {
                        return (true, Some(index));
                    }

                    // 2. If it's a recursive candidate, try matching with `**/` prefix
                    if is_recursive_candidate {
                        let recursive_pattern_str = format!("**/{}", pattern_str);
                        if let Ok(recursive_pattern) = Pattern::new(&recursive_pattern_str) {
                            if recursive_pattern.matches_path(relative_path) {
                                return (true, Some(index));
                            }
                        }
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
    use crate::config::DiscoveryConfig;
    use std::path::Path;

    fn create_test_config(process_last: Option<Vec<&str>>) -> DiscoveryConfig {
        let mut config = DiscoveryConfig::default_for_test();
        config.process_last = process_last.map(|v| v.iter().map(|s| s.to_string()).collect());
        config
    }

    #[test]
    fn test_no_last_patterns() {
        let config = create_test_config(None);
        let rel_path = Path::new("src/main.rs");
        assert_eq!(check_process_last(rel_path, &config), (false, None));
    }

    #[test]
    fn test_match_exact_filename_glob() {
        let config = create_test_config(Some(vec!["other.txt", "main.rs"]));
        let rel_path = Path::new("src/main.rs"); // Relative path includes directory
                                                 // "main.rs" glob matches recursively now
        assert_eq!(check_process_last(rel_path, &config), (true, Some(1)));

        let rel_path_root = Path::new("main.rs"); // Relative path is just filename
                                                  // "main.rs" glob DOES match "main.rs"
        assert_eq!(check_process_last(rel_path_root, &config), (true, Some(1)));
    }

    #[test]
    fn test_match_wildcard_filename_glob() {
        let config = create_test_config(Some(vec!["*.txt", "*.rs"]));
        let rel_path = Path::new("src/main.rs");
        // "*.rs" glob matches recursively
        assert_eq!(check_process_last(rel_path, &config), (true, Some(1)));

        let rel_path_txt = Path::new("docs/README.txt");
        // "*.txt" glob matches recursively
        assert_eq!(check_process_last(rel_path_txt, &config), (true, Some(0)));
    }

    #[test]
    fn test_match_relative_path_glob() {
        let config = create_test_config(Some(vec!["src/*.rs", "data/**/config.toml"]));
        let rel_path = Path::new("src/lib.rs");
        // "src/*.rs" glob DOES match "src/lib.rs"
        assert_eq!(check_process_last(rel_path, &config), (true, Some(0)));

        let rel_path_toml = Path::new("data/prod/config.toml");
        // "data/**/config.toml" glob DOES match "data/prod/config.toml"
        assert_eq!(check_process_last(rel_path_toml, &config), (true, Some(1)));

        let rel_path_toml_deep = Path::new("data/dev/nested/config.toml");
        // "data/**/config.toml" glob DOES match "data/dev/nested/config.toml"
        assert_eq!(
            check_process_last(rel_path_toml_deep, &config),
            (true, Some(1))
        );
    }

    #[test]
    fn test_no_match_glob() {
        let config = create_test_config(Some(vec!["lib.rs", "*.toml"]));
        let rel_path = Path::new("src/main.rs");
        assert_eq!(check_process_last(rel_path, &config), (false, None));
    }

    #[test]
    fn test_invalid_glob_pattern() {
        // Should log a warning but not panic, and not match
        let config = create_test_config(Some(vec!["[invalid", "*.rs"])); // First pattern is invalid
        let rel_path = Path::new("src/main.rs");
        // Should still match the second, valid pattern
        assert_eq!(check_process_last(rel_path, &config), (true, Some(1)));

        let rel_path_other = Path::new("src/other.txt");
        // Should not match anything
        assert_eq!(check_process_last(rel_path_other, &config), (false, None));
        // Check logs manually or using a test logger if needed to confirm warning
    }

    #[test]
    fn test_recursive_glob_matching_bug() {
        // This test confirms that the implementation now matches recursively
        // with simple globs like "*.rs", mimicking gitignore.
        let config = create_test_config(Some(vec!["*.rs"]));
        let rel_path = Path::new("src/main.rs");
        
        assert_eq!(check_process_last(rel_path, &config), (true, Some(0)), "Glob *.rs should match src/main.rs recursively");
    }
}
