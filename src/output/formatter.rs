// src/output/formatter.rs

use crate::config::Config;
use std::path::Path;

/// Formats a path for display in headers or summary list.
/// Applies backticks if configured.
pub fn format_path_for_display(path: &Path, config: &Config) -> String {
    // Use '/' as separator for consistent display, even on Windows
    let path_str = path.to_string_lossy().replace('\\', "/");
    if config.backticks {
        format!("`{}`", path_str)
    } else {
        path_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OutputDestination};
    use std::path::PathBuf;

    fn create_test_config(backticks: bool) -> Config {
        Config {
            input_path: PathBuf::from("."),
            base_path_display: ".".to_string(),
            input_is_file: false,
            backticks,
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
            output_destination: OutputDestination::Stdout,
            summary: false,
            counts: false,
            process_last: None,
            only_last: false,
            dry_run: false,
        }
    }

    #[test]
    fn test_format_no_backticks() {
        let config = create_test_config(false);
        let path = Path::new("src/main.rs");
        assert_eq!(format_path_for_display(path, &config), "src/main.rs");
    }

    #[test]
    fn test_format_with_backticks() {
        let config = create_test_config(true);
        let path = Path::new("src/main.rs");
        assert_eq!(format_path_for_display(path, &config), "`src/main.rs`");
    }

    #[test]
    #[cfg(windows)]
    fn test_format_windows_path_separator() {
        let config_no_ticks = create_test_config(false);
        let config_ticks = create_test_config(true);
        let path = Path::new("src\\main.rs"); // Windows-style separator
        assert_eq!(
            format_path_for_display(path, &config_no_ticks),
            "src/main.rs"
        ); // Replaced with /
        assert_eq!(
            format_path_for_display(path, &config_ticks),
            "`src/main.rs`"
        ); // Replaced with / and backticked
    }
}
