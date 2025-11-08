// src/output/formatter.rs

//! Provides helper functions for formatting parts of the output.

use crate::output::OutputOptions;
use std::path::Path;

/// Formats a path for display in headers or summary list.
///
/// This function ensures path separators are consistent (`/`) and applies
/// backticks for Markdown formatting if configured.
///
/// # Examples
/// ```
/// use dircat::output::formatter::format_path_for_display;
/// use dircat::output::OutputOptions;
/// use std::path::Path;
///
/// let opts_no_ticks = OutputOptions { backticks: false, filename_only_header: false, line_numbers: false, num_ticks: 3, summary: false, counts: false };
/// let opts_with_ticks = OutputOptions { backticks: true, filename_only_header: false, line_numbers: false, num_ticks: 3, summary: false, counts: false };
///
/// let path = Path::new("src/main.rs");
///
/// assert_eq!(format_path_for_display(path, &opts_no_ticks), "src/main.rs");
/// assert_eq!(format_path_for_display(path, &opts_with_ticks), "`src/main.rs`");
/// ```
pub fn format_path_for_display(path: &Path, opts: &OutputOptions) -> String {
    // Use '/' as separator for consistent display, even on Windows
    let path_str = path.to_string_lossy().replace('\\', "/");
    if opts.backticks {
        format!("`{}`", path_str)
    } else {
        path_str
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn create_test_opts(backticks: bool) -> OutputOptions {
        let mut opts = crate::output::tests::create_mock_config(false, false, false, false);
        opts.backticks = backticks;
        opts
    }

    #[test]
    fn test_format_no_backticks() {
        let opts = create_test_opts(false);
        let path = Path::new("src/main.rs");
        assert_eq!(format_path_for_display(path, &opts), "src/main.rs");
    }

    #[test]
    fn test_format_with_backticks() {
        let opts = create_test_opts(true);
        let path = Path::new("src/main.rs");
        assert_eq!(format_path_for_display(path, &opts), "`src/main.rs`");
    }

    #[test]
    #[cfg(windows)]
    fn test_format_windows_path_separator() {
        let opts_no_ticks = create_test_opts(false);
        let opts_ticks = create_test_opts(true);
        let path = Path::new("src\\main.rs"); // Windows-style separator
        assert_eq!(format_path_for_display(path, &opts_no_ticks), "src/main.rs"); // Replaced with /
        assert_eq!(format_path_for_display(path, &opts_ticks), "`src/main.rs`");
        // Replaced with / and backticked
    }
}
