// src/filtering/file_type.rs

use std::fs::Metadata;

/// Checks if the metadata belongs to a regular file.
///
/// This is a simple wrapper around `std::fs::Metadata::is_file`. It returns `false`
/// for directories, symbolic links, and other special file types.
///
/// # Examples
///
/// ```
/// # use std::fs;
/// # use dircat::filtering::is_file_type;
/// # use tempfile::tempdir;
/// # fn main() -> std::io::Result<()> {
/// let temp = tempdir()?;
/// let file_path = temp.path().join("file.txt");
/// let dir_path = temp.path().join("dir");
/// fs::write(&file_path, "content")?;
/// fs::create_dir(&dir_path)?;
///
/// let file_meta = fs::metadata(&file_path)?;
/// assert!(is_file_type(&file_meta));
///
/// let dir_meta = fs::metadata(&dir_path)?;
/// assert!(!is_file_type(&dir_meta));
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn is_file_type(metadata: &Metadata) -> bool {
    metadata.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_is_file() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("test_file.txt");
        fs::write(&file_path, "content")?;
        let metadata = fs::metadata(&file_path)?;
        assert!(is_file_type(&metadata));
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_is_not_file_dir() -> std::io::Result<()> {
        let temp = tempdir()?;
        let metadata = fs::metadata(temp.path())?;
        assert!(!is_file_type(&metadata));
        temp.close()?;
        Ok(())
    }

    // Note: Testing symlinks requires platform-specific code (unix/windows)
    // #[test]
    // #[cfg(unix)]
    // fn test_is_not_file_symlink() -> std::io::Result<()> {
    //     use std::os::unix::fs::symlink;
    //     let temp = tempdir()?;
    //     let file_path = temp.path().join("target.txt");
    //     fs::write(&file_path, "target")?;
    //     let link_path = temp.path().join("link.txt");
    //     symlink(&file_path, &link_path)?;
    //     // Use symlink_metadata to check the link itself, not the target
    //     let metadata = fs::symlink_metadata(&link_path)?;
    //     assert!(!is_file_type(&metadata)); // A symlink is not a regular file
    //     temp.close()?;
    //     Ok(())
    // }
}
