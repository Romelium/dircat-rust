// src/config/path_resolve.rs

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resolves the input path string to an absolute, canonicalized PathBuf.
pub(super) fn resolve_input_path(input_path_str: &str) -> Result<PathBuf> {
    let input_path = PathBuf::from(input_path_str);
    input_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve input path: '{}'", input_path_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_existing_dir() -> Result<()> {
        let temp = tempdir()?;
        let path_str = temp.path().to_str().unwrap();
        let resolved = resolve_input_path(path_str)?;
        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        assert!(resolved.is_dir());
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_resolve_existing_file() -> Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("test.txt");
        fs::write(&file_path, "content")?;
        let path_str = file_path.to_str().unwrap();
        let resolved = resolve_input_path(path_str)?;
        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        assert!(resolved.is_file());
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_resolve_non_existent_path() {
        let result = resolve_input_path("non_existent_path_for_testing_dircat");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to resolve input path"));
    }

    #[test]
    fn test_resolve_relative_path() -> Result<()> {
        // Create a file in the current dir to resolve relatively
        let filename = "relative_test_file_dircat.txt";
        fs::write(filename, "relative")?;
        let resolved = resolve_input_path(filename)?;
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with(filename));
        fs::remove_file(filename)?; // Clean up
        Ok(())
    }
}
