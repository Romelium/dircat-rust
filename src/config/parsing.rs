// src/config/parsing.rs

use anyhow::{Context, Result};
use byte_unit::Byte;
use regex::Regex;
use std::str::FromStr; // Import the FromStr trait

/// Parses the optional max size string into Option<u128>.
pub(super) fn parse_max_size(max_size_str: Option<String>) -> Result<Option<u128>> {
    // Changed return type to u128
    max_size_str
        .map(|s| {
            Byte::from_str(&s)
                .map(|b| b.as_u128()) // Use as_u128() which returns u128
                .with_context(|| format!("Invalid size format: '{}'", s))
        })
        .transpose() // Convert Option<Result<u128>> to Result<Option<u128>>
}

/// Compiles a vector of pattern strings into a vector of Regex objects.
pub(super) fn compile_regex_vec(
    patterns: Option<Vec<String>>,
    name: &str,
) -> Result<Option<Vec<Regex>>> {
    patterns
        .map(|vec| {
            vec.into_iter()
                .map(|p| Regex::new(&p).with_context(|| format!("Invalid {} regex: '{}'", name, p)))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()
}

/// Normalizes a vector of extension strings to lowercase.
pub(super) fn normalize_extensions(exts: Option<Vec<String>>) -> Option<Vec<String>> {
    exts.map(|v| v.into_iter().map(|s| s.to_lowercase()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_max_size() -> Result<()> {
        // Use 1000 for 'k' (SI prefix)
        assert_eq!(parse_max_size(Some("10k".to_string()))?, Some(10 * 1000));
        // Use 1024*1024 for 'MiB' (Binary prefix)
        assert_eq!(
            parse_max_size(Some("2MiB".to_string()))?,
            Some(2 * 1024 * 1024)
        );
        // Plain number is bytes
        assert_eq!(parse_max_size(Some("1024".to_string()))?, Some(1024));
        // None input
        assert_eq!(parse_max_size(None)?, None);
        Ok(())
    }

    #[test]
    fn test_parse_invalid_max_size() {
        let result = parse_max_size(Some("invalid".to_string()));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid size format"));
    }

    #[test]
    fn test_compile_valid_regex() -> Result<()> {
        let patterns = Some(vec!["^abc$".to_string(), ".*".to_string()]);
        let regexes = compile_regex_vec(patterns, "test")?;
        assert!(regexes.is_some());
        assert_eq!(regexes.unwrap().len(), 2);
        assert!(compile_regex_vec(None, "test")?.is_none());
        Ok(())
    }

    #[test]
    fn test_compile_invalid_regex() {
        let patterns = Some(vec!["[".to_string()]); // Invalid regex
        let result = compile_regex_vec(patterns, "test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid test regex"));
    }

    #[test]
    fn test_normalize_exts() {
        let exts = Some(vec!["Txt".to_string(), "RS".to_string()]);
        let normalized = normalize_extensions(exts);
        assert_eq!(normalized, Some(vec!["txt".to_string(), "rs".to_string()]));
        assert!(normalize_extensions(None).is_none());
    }
}
