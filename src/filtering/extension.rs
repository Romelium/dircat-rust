// src/filtering/extension.rs

use crate::config::Config; // Removed unused OutputDestination
use std::path::Path;

/// Checks if a path passes the include/exclude extension filters defined in Config.
/// Comparison is case-insensitive.
pub(crate) fn passes_extension_filters(path: &Path, config: &Config) -> bool {
    // Changed to pub(crate)
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
