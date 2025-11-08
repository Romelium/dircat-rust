// src/filtering/size.rs

use crate::discovery::DiscoveryOptions;
use std::fs::Metadata;

/// Checks if the file's size is within the configured limit.
#[inline]
///
/// This function compares the file's length from its `Metadata` against the
/// `max_size` value in the `Config`. If `max_size` is `None`, it always returns `true`.
///
/// # Examples
///
/// ```no_run
/// # use std::fs;
/// # use dircat::config::ConfigBuilder;
/// # use dircat::filtering::passes_size_filter;
/// # fn main() -> std::io::Result<()> {
/// let metadata = fs::metadata("Cargo.toml")?; // Assuming Cargo.toml is < 1000 bytes
///
/// let config_with_limit = ConfigBuilder::new().max_size("1000").build().unwrap();
/// let opts_with_limit = dircat::discovery::DiscoveryOptions::from(&config_with_limit);
/// assert!(passes_size_filter(&metadata, &opts_with_limit));
///
/// let config_no_limit = ConfigBuilder::new().build().unwrap();
/// let opts_no_limit = dircat::discovery::DiscoveryOptions::from(&config_no_limit);
/// assert!(passes_size_filter(&metadata, &opts_no_limit));
/// # Ok(())
/// # }
/// ```
pub fn passes_size_filter(metadata: &Metadata, opts: &DiscoveryOptions) -> bool {
    match opts.max_size {
        // Cast metadata.len() (u64) to u128 for comparison
        Some(max_size) => (metadata.len() as u128) <= max_size,
        None => true, // No limit set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_opts(max_size: Option<u128>) -> DiscoveryOptions<'static> {
        let mut builder = ConfigBuilder::new();
        if let Some(size) = max_size {
            builder = builder.max_size(size.to_string());
        }
        let config = Box::leak(Box::new(builder.build().unwrap()));
        DiscoveryOptions::from(&*config)
    }

    #[test]
    fn test_size_no_limit() -> std::io::Result<()> {
        let opts = create_test_opts(None);
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "12345")?; // 5 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(passes_size_filter(&metadata, &opts));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_within_limit() -> std::io::Result<()> {
        let opts = create_test_opts(Some(10)); // Limit 10 bytes
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "1234567890")?; // 10 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(passes_size_filter(&metadata, &opts));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_exceeds_limit() -> std::io::Result<()> {
        let opts = create_test_opts(Some(5)); // Limit 5 bytes
        let temp = tempdir()?;
        let file_path = temp.path().join("file.txt");
        fs::write(&file_path, "123456")?; // 6 bytes
        let metadata = fs::metadata(&file_path)?;
        assert!(!passes_size_filter(&metadata, &opts));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_size_zero_limit() -> std::io::Result<()> {
        let opts = create_test_opts(Some(0)); // Limit 0 bytes
        let temp = tempdir()?;
        let file_path_empty = temp.path().join("empty.txt");
        fs::write(&file_path_empty, "")?; // 0 bytes
        let metadata_empty = fs::metadata(&file_path_empty)?;
        assert!(passes_size_filter(&metadata_empty, &opts));

        let file_path_nonempty = temp.path().join("nonempty.txt");
        fs::write(&file_path_nonempty, "a")?; // 1 byte
        let metadata_nonempty = fs::metadata(&file_path_nonempty)?;
        assert!(!passes_size_filter(&metadata_nonempty, &opts));

        temp.close()?;
        Ok(())
    }
}
