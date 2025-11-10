// src/filtering/text_detection.rs

use content_inspector::ContentType;
use std::{fs::File, io::Read, path::Path, str};

// Define a reasonable buffer size for content type detection
const READ_BUFFER_SIZE: usize = 1024;

/// Checks if a byte buffer is likely text-based.
///
/// This is the core logic used by both `is_likely_text` and the main processing loop.
/// It uses `content_inspector` to perform a heuristic check and then verifies
/// UTF-8 validity if the content type is ambiguous.
///
/// # Examples
/// ```
/// use dircat::filtering::is_likely_text_from_buffer;
///
/// let text_buffer = b"This is valid UTF-8 text.";
/// assert!(is_likely_text_from_buffer(text_buffer));
///
/// let binary_buffer = b"This contains a null byte \0.";
/// assert!(!is_likely_text_from_buffer(binary_buffer));
///
/// let invalid_utf8_buffer = &[0x48, 0x65, 0x6c, 0x6c, 0x80, 0x6f]; // "Hell\x80o"
/// assert!(!is_likely_text_from_buffer(invalid_utf8_buffer));
/// ```
pub fn is_likely_text_from_buffer(buffer_slice: &[u8]) -> bool {
    // Inspect the bytes read
    let content_type = content_inspector::inspect(buffer_slice);

    // Consider it text ONLY if explicitly detected as UTF_8_BOM,
    // OR if detected as UTF_8 AND the buffer slice is actually valid UTF-8.
    // All other types (BINARY, etc.) are treated as non-text.
    match content_type {
        ContentType::UTF_8_BOM => true,
        ContentType::UTF_8 => str::from_utf8(buffer_slice).is_ok(), // Verify UTF-8 validity
        ContentType::BINARY => false,
        // Handle other potential future types conservatively as non-text
        _ => false,
    }
}

/// Checks if the file content is likely text-based by reading its head.
///
/// This function is suitable for checks where the file content is not already in memory
/// (e.g., during a dry run). It reads the first 1024 bytes of the file to perform
/// the heuristic check.
///
/// # Returns
/// `Ok(true)` if likely text, `Ok(false)` otherwise.
///
/// # Errors
/// Returns an `Err` on I/O error (e.g., file not found, permission denied).
///
/// # Examples
/// ```
/// # use std::fs;
/// # use dircat::filtering::is_likely_text;
/// # use tempfile::tempdir;
/// # fn main() -> std::io::Result<()> {
/// let temp = tempdir()?;
/// let text_file = temp.path().join("text.txt");
/// let binary_file = temp.path().join("binary.bin");
///
/// fs::write(&text_file, "Hello, world!")?;
/// assert!(is_likely_text(&text_file)?);
///
/// fs::write(&binary_file, b"binary\0data")?;
/// assert!(!is_likely_text(&binary_file)?);
/// # Ok(())
/// # }
/// ```
pub fn is_likely_text(path: &Path) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buffer = [0; READ_BUFFER_SIZE];
    let bytes_read = file.read(&mut buffer)?;
    let buffer_slice = &buffer[..bytes_read];

    Ok(is_likely_text_from_buffer(buffer_slice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    use tempfile::tempdir;

    // --- Tests for is_likely_text_from_buffer ---
    #[test]
    fn test_buffer_detect_utf8_text() {
        let buffer = b"This is plain UTF-8 text.";
        assert!(is_likely_text_from_buffer(buffer));
    }

    #[test]
    fn test_buffer_detect_utf8_bom_text() {
        let buffer = &[0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert!(is_likely_text_from_buffer(buffer));
    }

    #[test]
    fn test_buffer_detect_binary_null_byte() {
        let buffer = b"Binary data with a \0 null byte.";
        assert!(!is_likely_text_from_buffer(buffer));
    }

    #[test]
    fn test_buffer_detect_invalid_utf8_sequence() {
        let buffer = &[0x48, 0x65, 0x6c, 0x6c, 0x80, 0x6f]; // "Hell\x80o"
        assert!(!is_likely_text_from_buffer(buffer));
    }

    // --- Tests for is_likely_text (file-based) ---
    #[test]
    fn test_detect_utf8_text() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("utf8.txt");
        fs::write(&file_path, "This is plain UTF-8 text.")?;
        assert!(is_likely_text(&file_path)?); // Should detect UTF_8 and be valid -> true
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_utf8_bom_text() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("utf8_bom.txt");
        let mut file = fs::File::create(&file_path)?;
        // UTF-8 BOM bytes
        file.write_all(&[0xEF, 0xBB, 0xBF])?;
        file.write_all(b"Text with UTF-8 BOM.")?;
        drop(file);
        assert!(is_likely_text(&file_path)?); // Should detect UTF_8_BOM -> true
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_binary_null_byte() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("binary_null.bin");
        fs::write(&file_path, b"Binary data with a \0 null byte.")?;
        // content_inspector detects this as BINARY
        assert!(!is_likely_text(&file_path)?); // BINARY -> false
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_binary_high_bytes() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("binary_high.bin");
        // Write bytes outside typical text ranges (including invalid UTF-8)
        fs::write(&file_path, [0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD])?;
        // content_inspector might detect this as UTF_8 (incorrectly), but str::from_utf8 will fail
        assert!(!is_likely_text(&file_path)?); // UTF_8 but invalid -> false
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_empty_file() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("empty.txt");
        fs::write(&file_path, "")?;
        // content_inspector classifies empty as UTF-8, and "" is valid UTF-8
        assert!(is_likely_text(&file_path)?); // UTF_8 and valid -> true
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_png_file() -> std::io::Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("image.png");
        // PNG magic bytes
        fs::write(&file_path, [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])?;
        // content_inspector detects this as BINARY
        assert!(!is_likely_text(&file_path)?); // BINARY -> false
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_non_existent_file() {
        let path = Path::new("non_existent_file_for_text_detection.txt");
        let result = is_likely_text(path);
        assert!(result.is_err()); // Expect an I/O error
    }

    #[test]
    fn test_detect_ascii_text() -> std::io::Result<()> {
        // Test pure ASCII which should be detected as UTF_8 and be valid
        let temp = tempdir()?;
        let file_path = temp.path().join("ascii.txt");
        fs::write(&file_path, "Just plain ASCII.")?;
        assert!(is_likely_text(&file_path)?); // UTF_8 and valid -> true
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_detect_invalid_utf8_sequence() -> std::io::Result<()> {
        // Test a sequence that content_inspector might call UTF_8 but is invalid
        let temp = tempdir()?;
        let file_path = temp.path().join("invalid_utf8.txt");
        // 0x80 is an invalid UTF-8 start byte
        fs::write(&file_path, [0x48, 0x65, 0x6c, 0x6c, 0x80, 0x6f])?; // "Hell\x80o"
        assert!(!is_likely_text(&file_path)?); // UTF_8 but invalid -> false
        temp.close()?;
        Ok(())
    }
}
