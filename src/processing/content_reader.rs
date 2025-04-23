// src/processing/content_reader.rs

use crate::errors::io_error_with_path;
use anyhow::{Context, Result};
use std::{fs, path::Path};

/// Reads the entire content of a file into a String.
/// Wraps I/O errors with context.
pub(super) fn read_file_content(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|e| io_error_with_path(e, path))
        .with_context(|| format!("Failed to read file content: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_valid_file() -> Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("test.txt");
        let content = "Hello, dircat!";
        fs::write(&file_path, content)?;

        let read_content = read_file_content(&file_path)?;
        assert_eq!(read_content, content);

        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_read_empty_file() -> Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("empty.txt");
        fs::write(&file_path, "")?;

        let read_content = read_file_content(&file_path)?;
        assert_eq!(read_content, "");

        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_read_non_existent_file() {
        let path = Path::new("non_existent_file_for_dircat_test.txt");
        let result = read_file_content(path);
        assert!(result.is_err());
        let err_string = result.unwrap_err().to_string();
        assert!(err_string.contains("Failed to read file content"));
        assert!(err_string.contains("non_existent_file"));
    }

    // Test reading non-UTF8 file (optional, depends on desired behavior)
    // #[test]
    // fn test_read_non_utf8_file() -> Result<()> {
    //     let temp = tempdir()?;
    //     let file_path = temp.path().join("binary.dat");
    //     // Write invalid UTF-8 sequence
    //     fs::write(&file_path, &[0x80, 0x81, 0x82])?;
    //
    //     let result = read_file_content(&file_path);
    //     assert!(result.is_err()); // read_to_string should fail on invalid UTF-8
    //     assert!(result.unwrap_err().to_string().contains("stream did not contain valid UTF-8"));
    //
    //     temp.close()?;
    //     Ok(())
    // }
}
