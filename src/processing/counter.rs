// src/processing/counter.rs

use crate::core_types::FileCounts;

/// Calculates line, character, and word counts for a string slice.
///
/// This function assumes the input is text. For binary content that has been lossily
/// converted to a string, only the `characters` (byte) count is reliable.
/// The calculation rules are as follows:
///
/// - **Lines**: The number of lines, calculated by counting newline (`\n`) characters.
/// - **Characters**: The total number of bytes in the string.
/// - **Words**: The number of words, calculated by splitting the content by whitespace.
///
/// # Examples
///
/// ```
/// use dircat::processing::calculate_counts;
///
/// let content = "Hello, world!\nThis is a test.";
/// let counts = calculate_counts(content);
/// assert_eq!(counts.lines, 2);
/// assert_eq!(counts.words, 6);
/// assert_eq!(counts.characters, 29);
/// ```
#[inline]
pub fn calculate_counts(content: &str) -> FileCounts {
    FileCounts {
        lines: content.lines().count(),
        characters: content.len(), // Number of bytes
        words: content.split_whitespace().count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counts_basic() {
        let content = "One two three.\nFour five.\n";
        let counts = calculate_counts(content);
        assert_eq!(counts.lines, 2);
        assert_eq!(counts.characters, 26); // Includes newline chars
        assert_eq!(counts.words, 5);
    }

    #[test]
    fn test_counts_empty_string() {
        let content = "";
        let counts = calculate_counts(content);
        assert_eq!(counts.lines, 0);
        assert_eq!(counts.characters, 0);
        assert_eq!(counts.words, 0);
    }

    #[test]
    fn test_counts_whitespace_only() {
        let content = "  \n\t \n ";
        let counts = calculate_counts(content);
        assert_eq!(counts.lines, 3); // lines() counts lines separated by \n
        assert_eq!(counts.characters, 7);
        assert_eq!(counts.words, 0); // split_whitespace finds no words
    }

    #[test]
    fn test_counts_no_trailing_newline() {
        let content = "One two";
        let counts = calculate_counts(content);
        assert_eq!(counts.lines, 1);
        assert_eq!(counts.characters, 7);
        assert_eq!(counts.words, 2);
    }

    #[test]
    fn test_counts_multiple_spaces() {
        let content = "One   two \t three";
        let counts = calculate_counts(content);
        assert_eq!(counts.lines, 1);
        assert_eq!(counts.characters, 17);
        assert_eq!(counts.words, 3); // split_whitespace handles multiple spaces
    }
}
