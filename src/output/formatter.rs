// src/output/formatter.rs

//! Provides helper functions for formatting parts of the output.

use crate::config::Config;
use std::path::Path;

/// Formats a path for display in headers or summary list.
///
/// This function ensures path separators are consistent (`/`) and applies
/// backticks for Markdown formatting if configured.
///
/// # Examples
/// ```
/// use dircat::output::formatter::format_path_for_display;
/// use dircat::config::Config;
/// use std::path::Path;
///
/// let mut config_no_ticks = Config::new_for_test();
/// config_no_ticks.backticks = false;
///
/// let mut config_with_ticks = Config::new_for_test();
/// config_with_ticks.backticks = true;
///
/// let path = Path::new("src/main.rs");
///
/// assert_eq!(format_path_for_display(path, &config_no_ticks), "src/main.rs");
/// assert_eq!(format_path_for_display(path, &config_with_ticks), "`src/main.rs`");
/// ```
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
    use crate::config::Config;
    use std::path::Path;

    fn create_test_config(backticks: bool) -> Config {
        let mut config = Config::new_for_test();
        config.backticks = backticks;
        config
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
