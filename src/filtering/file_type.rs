// src/filtering/file_type.rs

use std::fs::Metadata;

/// Checks if the metadata belongs to a regular file.
#[inline]
pub(crate) fn is_file_type(metadata: &Metadata) -> bool {
    // Changed to pub(crate)
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
