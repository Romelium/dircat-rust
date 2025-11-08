// src/filtering/extension.rs

use crate::discovery::DiscoveryOptions;
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
/// use dircat::config::ConfigBuilder;
/// use dircat::filtering::passes_extension_filters;
/// use std::path::Path;
///
/// // --- Setup ---
/// let config_none = ConfigBuilder::new().build().unwrap();
/// let opts_none = dircat::discovery::DiscoveryOptions::from(&config_none);
/// let path_rs = Path::new("src/main.rs");
/// let path_toml = Path::new("Cargo.toml");
/// let path_lock = Path::new("Cargo.lock");
/// let path_no_ext = Path::new("Makefile");
///
/// // --- Case 1: No filters ---
/// assert!(passes_extension_filters(path_rs, &opts_none));
/// assert!(passes_extension_filters(path_no_ext, &opts_none));
///
/// // --- Case 2: Include filter ---
/// let config_include = ConfigBuilder::new().extensions(vec!["rs".to_string(), "toml".to_string()]).build().unwrap();
/// let opts_include = dircat::discovery::DiscoveryOptions::from(&config_include);
/// assert!(passes_extension_filters(path_rs, &opts_include));
/// assert!(passes_extension_filters(path_toml, &opts_include));
/// assert!(!passes_extension_filters(path_lock, &opts_include)); // Not in include list
/// assert!(!passes_extension_filters(path_no_ext, &opts_include)); // No extension, fails include
///
/// // --- Case 3: Exclude filter takes precedence ---
/// let config_exclude = ConfigBuilder::new().extensions(vec!["rs".to_string(), "toml".to_string()]).exclude_extensions(vec!["toml".to_string()]).build().unwrap();
/// let opts_exclude = dircat::discovery::DiscoveryOptions::from(&config_exclude);
/// assert!(passes_extension_filters(path_rs, &opts_exclude)); // Still included
/// assert!(!passes_extension_filters(path_toml, &opts_exclude)); // Excluded, even though it's in the include list
/// ```
pub fn passes_extension_filters(path: &Path, opts: &DiscoveryOptions) -> bool {
    let extension = path
        .extension()
        .and_then(|os_str| os_str.to_str())
        .map(|s| s.to_lowercase()); // Compare case-insensitively

    // 1. Check exclude extensions first
    if let Some(ref exclude_exts) = opts.exclude_extensions {
        if let Some(ref ext) = extension {
            if exclude_exts.contains(ext) {
                return false; // Excluded
            }
        }
    }

    // 2. Check include extensions if specified
    if let Some(ref include_exts) = opts.extensions {
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
    use crate::config::ConfigBuilder;
    use std::path::Path;

    // Helper to create a minimal Config for testing filters
    fn create_test_opts(
        extensions: Option<Vec<&str>>,
        exclude_extensions: Option<Vec<&str>>,
    ) -> DiscoveryOptions<'static> {
        let mut builder = ConfigBuilder::new();
        if let Some(exts) = extensions {
            builder = builder.extensions(exts.iter().map(|s| s.to_string()).collect());
        }
        if let Some(exts) = exclude_extensions {
            builder = builder.exclude_extensions(exts.iter().map(|s| s.to_string()).collect());
        }
        let config = Box::leak(Box::new(builder.build().unwrap()));
        DiscoveryOptions::from(&*config)
    }

    #[test]
    fn test_ext_no_filters() {
        let opts = create_test_opts(None, None);
        assert!(passes_extension_filters(Path::new("file.txt"), &opts));
        assert!(passes_extension_filters(Path::new("file.rs"), &opts));
        assert!(passes_extension_filters(Path::new("file"), &opts)); // No extension
    }

    #[test]
    fn test_ext_include() {
        let opts = create_test_opts(Some(vec!["txt", "md"]), None);
        assert!(passes_extension_filters(Path::new("file.txt"), &opts));
        assert!(passes_extension_filters(Path::new("FILE.MD"), &opts)); // Case insensitive
        assert!(!passes_extension_filters(Path::new("file.rs"), &opts));
        assert!(!passes_extension_filters(Path::new("file"), &opts)); // No extension fails include
    }

    #[test]
    fn test_ext_exclude() {
        let opts = create_test_opts(None, Some(vec!["log", "tmp"]));
        assert!(passes_extension_filters(Path::new("file.txt"), &opts));
        assert!(!passes_extension_filters(Path::new("file.log"), &opts));
        assert!(!passes_extension_filters(Path::new("file.TMP"), &opts)); // Case insensitive
        assert!(passes_extension_filters(Path::new("file"), &opts)); // No extension passes exclude
    }

    #[test]
    fn test_ext_include_and_exclude() {
        let opts = create_test_opts(Some(vec!["txt", "md"]), Some(vec!["bak", "md"])); // Include txt/md, exclude bak/md
        assert!(passes_extension_filters(Path::new("file.txt"), &opts)); // Included
        assert!(!passes_extension_filters(Path::new("file.md"), &opts)); // Excluded (exclude takes precedence)
        assert!(!passes_extension_filters(Path::new("file.bak"), &opts)); // Excluded
        assert!(!passes_extension_filters(Path::new("file.rs"), &opts)); // Not included
    }
}
