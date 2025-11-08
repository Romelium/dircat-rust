// src/filtering/extension.rs

use crate::config::Config;
use std::path::Path;

/// Checks if a path passes the include/exclude extension filters defined in `Config`.
///
/// The filtering logic follows these rules in order:
///
/// 1.  **Exclusion Precedence:** If `config.exclude_extensions` is `Some` and contains the
///     file's extension, the function returns `false`. This check is performed first
///     and takes priority over the inclusion filter.
/// 2.  **Inclusion Requirement:** If `config.extensions` is `Some`, the file must have an
///     extension, and that extension must be present in the include list. If it's not,
///     the function returns `false`.
/// 3.  **Default Pass:** If the file is not filtered out by the above rules, the function
///     returns `true`.
///
/// The comparison is always case-insensitive.
///
/// # Examples
///
/// ```
/// use dircat::config::Config;
/// use dircat::filtering::passes_extension_filters;
/// use std::path::Path;
///
/// // --- Setup ---
/// let mut config = Config::new_for_test();
/// let path_rs = Path::new("src/main.rs");
/// let path_toml = Path::new("Cargo.toml");
/// let path_lock = Path::new("Cargo.lock");
/// let path_no_ext = Path::new("Makefile");
///
/// // --- Case 1: No filters ---
/// assert!(passes_extension_filters(path_rs, &config));
/// assert!(passes_extension_filters(path_no_ext, &config));
///
/// // --- Case 2: Include filter ---
/// config.extensions = Some(vec!["rs".to_string(), "toml".to_string()]);
/// assert!(passes_extension_filters(path_rs, &config));
/// assert!(passes_extension_filters(path_toml, &config));
/// assert!(!passes_extension_filters(path_lock, &config)); // Not in include list
/// assert!(!passes_extension_filters(path_no_ext, &config)); // No extension, fails include
///
/// // --- Case 3: Exclude filter takes precedence ---
/// config.exclude_extensions = Some(vec!["toml".to_string()]);
/// assert!(passes_extension_filters(path_rs, &config)); // Still included
/// assert!(!passes_extension_filters(path_toml, &config)); // Excluded, even though it's in the include list
/// ```
pub fn passes_extension_filters(path: &Path, config: &Config) -> bool {
    let extension = path
        .extension()
        .and_then(|os_str| os_str.to_str())
        .map(|s| s.to_lowercase()); // Compare case-insensitively

    // 1. Check exclude extensions first
    if let Some(ref exclude_exts) = config.exclude_extensions {
        if let Some(ref ext) = extension {
            if exclude_exts.contains(ext) {
                return false; // Excluded
            }
        }
    }

    // 2. Check include extensions if specified
    if let Some(ref include_exts) = config.extensions {
        if let Some(ref ext) = extension {
            if !include_exts.contains(ext) {
                return false; // Not in the include list
            }
        } else {
            return false; // No extension, but include list specified
        }
    }

    true // Passes if not excluded and meets include criteria (if any)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config; // Need a mock or partial Config
    use std::path::Path;

    // Helper to create a minimal Config for testing filters
    fn create_test_config(
        extensions: Option<Vec<&str>>,
        exclude_extensions: Option<Vec<&str>>,
    ) -> Config {
        let mut config = Config::new_for_test();
        config.extensions = extensions.map(|v| v.iter().map(|s| s.to_lowercase()).collect());
        config.exclude_extensions =
            exclude_extensions.map(|v| v.iter().map(|s| s.to_lowercase()).collect());
        config
    }

    #[test]
    fn test_ext_no_filters() {
        let config = create_test_config(None, None);
        assert!(passes_extension_filters(Path::new("file.txt"), &config));
        assert!(passes_extension_filters(Path::new("file.rs"), &config));
        assert!(passes_extension_filters(Path::new("file"), &config)); // No extension
    }

    #[test]
    fn test_ext_include() {
        let config = create_test_config(Some(vec!["txt", "md"]), None);
        assert!(passes_extension_filters(Path::new("file.txt"), &config));
        assert!(passes_extension_filters(Path::new("FILE.MD"), &config)); // Case insensitive
        assert!(!passes_extension_filters(Path::new("file.rs"), &config));
        assert!(!passes_extension_filters(Path::new("file"), &config)); // No extension fails include
    }

    #[test]
    fn test_ext_exclude() {
        let config = create_test_config(None, Some(vec!["log", "tmp"]));
        assert!(passes_extension_filters(Path::new("file.txt"), &config));
        assert!(!passes_extension_filters(Path::new("file.log"), &config));
        assert!(!passes_extension_filters(Path::new("file.TMP"), &config)); // Case insensitive
        assert!(passes_extension_filters(Path::new("file"), &config)); // No extension passes exclude
    }

    #[test]
    fn test_ext_include_and_exclude() {
        let config = create_test_config(Some(vec!["txt", "md"]), Some(vec!["bak", "md"])); // Include txt/md, exclude bak/md
        assert!(passes_extension_filters(Path::new("file.txt"), &config)); // Included
        assert!(!passes_extension_filters(Path::new("file.md"), &config)); // Excluded (exclude takes precedence)
        assert!(!passes_extension_filters(Path::new("file.bak"), &config)); // Excluded
        assert!(!passes_extension_filters(Path::new("file.rs"), &config)); // Not included
    }
}
