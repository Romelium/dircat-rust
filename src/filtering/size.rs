// src/filtering/size.rs

use crate::config::Config;
use std::fs::Metadata;

/// Checks if the file's size is within the configured limit.
#[inline]
pub(crate) fn passes_size_filter(metadata: &Metadata, config: &Config) -> bool {
    // Changed to pub(crate)
    match config.max_size {
        // Cast metadata.len() (u64) to u128 for comparison
        Some(max_size) => (metadata.len() as u128) <= max_size,
        None => true, // No limit set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_config(max_size: Option<u128>) -> Config {
        // Changed to u128
        let mut config = Config::new_for_test();
        config.max_size = max_size;
        config
    }

    #[test]
    fn test_size_no_limit() -> std::io::Result<()> {
        let config = create_test_config(None);
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "12345")?; // 5 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(passes_size_filter(&metadata, &config));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_within_limit() -> std::io::Result<()> {
        let config = create_test_config(Some(10)); // Limit 10 bytes
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "1234567890")?; // 10 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(passes_size_filter(&metadata, &config));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_exceeds_limit() -> std::io::Result<()> {
        let config = create_test_config(Some(5)); // Limit 5 bytes
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "123456")?; // 6 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(!passes_size_filter(&metadata, &config));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_zero_limit() -> std::io::Result<()> {
        let config = create_test_config(Some(0)); // Limit 0 bytes
        let temp = tempdir()?;
        let file_path_empty = temp.path().join("empty.txt");
        fs::write(&file_path_empty, "")?; // 0 bytes
        let metadata_empty = fs::metadata(&file_path_empty)?;
        assert!(passes_size_filter(&metadata_empty, &config));

        let file_path_nonempty = temp.path().join("nonempty.txt");
        fs::write(&file_path_nonempty, "a")?; // 1 byte
        let metadata_nonempty = fs::metadata(&file_path_nonempty)?;
        assert!(!passes_size_filter(&metadata_nonempty, &config));

        temp.close()?;
        Ok(())
    }
}
