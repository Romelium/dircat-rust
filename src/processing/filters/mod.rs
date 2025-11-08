//! Provides a trait and implementations for transforming file content.

use std::fmt;

mod comments;
mod empty_lines;

// Re-export the standalone functions
pub use comments::remove_comments;
pub use empty_lines::remove_empty_lines;

/// A trait for content transformation filters.
///
/// Filters are applied sequentially to the content of each text file.
pub trait ContentFilter: Send + Sync {
    /// Applies the filter to the given content string.
    fn apply(&self, content: &str) -> String;
    /// Returns a descriptive name for the filter.
    fn name(&self) -> &'static str;
}

// Implement Debug manually for Box<dyn ContentFilter> by using the name method.
impl fmt::Debug for Box<dyn ContentFilter> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentFilter").field(&self.name()).finish()
    }
}

// --- Filter Implementations ---

/// Filter to remove C/C++ style comments.
#[derive(Debug)]
pub struct RemoveCommentsFilter;

impl ContentFilter for RemoveCommentsFilter {
    fn apply(&self, content: &str) -> String {
        comments::remove_comments(content)
    }
    fn name(&self) -> &'static str {
        "RemoveCommentsFilter"
    }
}

/// Filter to remove lines containing only whitespace.
#[derive(Debug)]
pub struct RemoveEmptyLinesFilter;

impl ContentFilter for RemoveEmptyLinesFilter {
    fn apply(&self, content: &str) -> String {
        empty_lines::remove_empty_lines(content)
    }
    fn name(&self) -> &'static str {
        "RemoveEmptyLinesFilter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Trait Implementation Tests ---

    #[test]
    fn test_remove_comments_filter_apply() {
        let filter = RemoveCommentsFilter;
        let input = "code // comment\nmore code";
        let expected = "code\nmore code";
        assert_eq!(filter.apply(input), expected);
    }

    #[test]
    fn test_remove_empty_lines_filter_apply() {
        let filter = RemoveEmptyLinesFilter;
        let input = "Line 1\n\n  \nLine 4\n";
        let expected = "Line 1\nLine 4";
        assert_eq!(filter.apply(input), expected);
    }

    // --- remove_empty_lines tests ---
    #[test]
    fn test_remove_empty() {
        let input = "Line 1\n\n  \nLine 4\n";
        let expected = "Line 1\nLine 4";
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_empty_no_empty_lines() {
        let input = "Line 1\nLine 2";
        let expected = "Line 1\nLine 2";
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_empty_all_empty() {
        let input = "\n  \n\t\n";
        let expected = "";
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_empty_trailing_newlines() {
        let input = "Line 1\nLine 2\n\n";
        let expected = "Line 1\nLine 2"; // Trailing empty lines removed
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_empty_leading_newlines() {
        let input = "\n\nLine 1\nLine 2";
        let expected = "Line 1\nLine 2"; // Leading empty lines removed
        assert_eq!(remove_empty_lines(input), expected);
    }

    #[test]
    fn test_remove_empty_windows_newlines() {
        // Note: .lines() handles \r\n, but join("\n") only uses \n
        let input = "Line 1\r\n\r\n  \r\nLine 4\r\n";
        let expected = "Line 1\nLine 4";
        assert_eq!(remove_empty_lines(input), expected);
    }

    // --- remove_comments tests ---
    #[test]
    fn test_remove_line_comment_simple() {
        let input = "code // comment\nmore code";
        let expected = "code\nmore code"; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_remove_line_comment_no_newline() {
        let input = "code // comment";
        let expected = "code"; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_remove_block_comment_simple() {
        let input = "code /* comment */ more code";
        let expected = "code  more code"; // Space where comment was remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_remove_block_comment_multiline() {
        let input = "code /* comment\n more comment */ more code";
        let expected = "code  more code";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_remove_block_comment_with_stars() {
        let input = "code /**** comment ****/ more code";
        let expected = "code  more code";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_remove_block_comment_at_end() {
        let input = "code /* comment */";
        let expected = "code"; // Trailing space removed by trim_end, final trim handles if only space remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_comment_markers_in_strings() {
        let input = r#"let s = "// not a comment"; /* also " not start */"#;
        let expected = r#"let s = "// not a comment";"#; // Trailing space removed by trim_end, final trim handles if only space remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_comment_markers_in_chars() {
        let input = r#"let c = '/'; // char comment"#;
        let expected = r#"let c = '/';"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);

        let input2 = r#"let c = '*'; /* char comment */"#;
        let expected2 = r#"let c = '*';"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input2), expected2);
    }

    #[test]
    fn test_escaped_quotes_in_strings() {
        let input = r#"let s = "string with \" quote"; // comment"#;
        let expected = r#"let s = "string with \" quote";"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_escaped_slash_before_comment() {
        let input = r#"let path = "\\\\server\\share"; // comment"#;
        let expected = r#"let path = "\\\\server\\share";"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_division_operator() {
        let input = "a = b / c; // divide\nx = y / *p; /* ptr divide */";
        let expected = "a = b / c;\nx = y / *p;"; // Trailing spaces removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let expected = "";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_only_line_comment() {
        let input = "// only comment";
        let expected = "";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_only_line_comment_with_newline() {
        let input = "// only comment\n";
        let expected = ""; // Newline removed by final trim
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_only_block_comment() {
        let input = "/* only comment */";
        let expected = "";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_only_block_comment_multiline() {
        let input = "/* only \n comment */";
        let expected = "";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_block_comment_unterminated() {
        // Should consume until the end
        let input = "code /* comment";
        let expected = "code"; // Trailing space removed by trim_end, final trim handles if only space remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_ends_with_slash() {
        let input = "code /";
        let expected = "code /";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_ends_with_star_in_block() {
        let input = "code /* comment *";
        let expected = "code"; // Trailing space removed by trim_end, final trim handles if only space remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_adjacent_block_comments() {
        let input = "code /* first */ /* second */";
        let expected = "code"; // Trailing space removed by trim_end, final trim handles if only space remains
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_adjacent_line_comments() {
        let input = "code // first \n// second\nend";
        let expected = "code\n\nend"; // Trailing spaces removed, newlines preserved
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_block_comment_inside_line_comment() {
        // Line comment takes precedence
        let input = "code // line /* block */ comment\nend";
        let expected = "code\nend"; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_line_comment_inside_block_comment() {
        // Block comment takes precedence
        let input = "code /* block // line \n comment */ end";
        let expected = "code  end";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_string_with_comment_markers_and_escapes() {
        let input =
            r#"str = "/* not comment */ // also not comment \" escaped quote"; // real comment"#;
        let expected = r#"str = "/* not comment */ // also not comment \" escaped quote";"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_char_with_comment_markers_and_escapes() {
        let input = r#"
let c1 = '/'; // comment 1
let c2 = '*'; /* comment 2 */
let c3 = '\\'; // comment 3
let c4 = '\''; // comment 4
"#;
        // Expected: Keep code, remove comments, trim trailing spaces.
        // The leading/trailing newlines from the input string literal are handled by the final trim.
        let expected = "let c1 = '/';\nlet c2 = '*';\nlet c3 = '\\\\';\nlet c4 = '\\'';";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_mixed_comments_and_code() {
        let input = r#"
        int main() { // start main
            /* block comment
               here */
            printf("Hello // World\n"); /* Print */
            // return 0;
            return 1; /* Success? */
        } // end main"#;
        // Expected: Code preserved, comments removed, trailing spaces removed.
        // Leading/trailing empty lines handled by final trim.
        // Lines that contained only comments become empty lines after trim_end(), resulting in \n\n.
        let expected = "int main() {\n\n            printf(\"Hello // World\\n\");\n\n            return 1;\n        }";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_tricky_slashes_and_stars() {
        let input = "a = b / *p; // divide by pointer value\n c = d */e; /* incorrect comment? */";
        // Expected: Comments removed, trailing spaces removed.
        let expected = "a = b / *p;\n c = d */e;"; // Treats */ as normal code here
        assert_eq!(remove_comments(input), expected);

        let input2 = "a = b / * p * / c; /* comment */"; // Pointer arithmetic, then comment
        let expected2 = "a = b / * p * / c;"; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input2), expected2);
    }

    #[test]
    fn test_slash_then_quote() {
        let input = r#"x = y / "/"; // divide by string"#;
        let expected = r#"x = y / "/";"#; // Trailing space removed by trim_end
        assert_eq!(remove_comments(input), expected);
    }
}
