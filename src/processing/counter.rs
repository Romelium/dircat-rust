// src/processing/counter.rs

use crate::core_types::FileCounts;

/// Calculates line, character (byte), and word counts for the given content.
/// Assumes the input is text. For binary files, only character count is reliable.
/// Words are split by whitespace.
#[inline]
pub(super) fn calculate_counts(content: &str) -> FileCounts {
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
