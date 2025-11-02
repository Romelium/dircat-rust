//! Provides functions for transforming file content.
//!
//! This module contains standalone filters that can be applied to string content,
//! such as removing comments or empty lines.

use log::debug;

/// Removes C/C++ style comments (// and /* ... */) using a state machine.
///
/// This function correctly handles comments within string and character literals,
/// as well as escaped characters. After removing comments, it trims trailing
/// whitespace from each line and then trims leading/trailing whitespace from the
/// entire resulting string.
///
/// # Examples
/// ```
/// use dircat::processing::filters::remove_comments;
///
/// let code = r#"
///     let url = "https://example.com"; // A URL
///     /* Block comment */
///     let value = 10;
/// "#;
///
/// // The line with the block comment becomes an empty line, and indentation is preserved.
/// // The final .trim() in remove_comments removes the leading/trailing newlines from the
/// // original string literal.
/// let expected = r#"let url = "https://example.com";
///
///     let value = 10;"#;
///
/// assert_eq!(remove_comments(code).trim(), expected);
/// ```
pub fn remove_comments(content: &str) -> String {
    enum State {
        Normal,
        MaybeSlash,           // Seen '/'
        LineComment,          // Seen '//'
        BlockComment,         // Seen '/*'
        MaybeEndBlockComment, // Seen '*' inside block comment
        StringLiteral,        // Seen '"'
        StringEscape,         // Seen '\' inside string
        CharLiteral,          // Seen '\''
        CharEscape,           // Seen '\' inside char
    }

    let mut result = String::with_capacity(content.len());
    let mut state = State::Normal;
    let chars = content.chars().peekable();

    for c in chars {
        match state {
            State::Normal => match c {
                '/' => state = State::MaybeSlash,
                '"' => {
                    state = State::StringLiteral;
                    result.push(c);
                }
                '\'' => {
                    state = State::CharLiteral;
                    result.push(c);
                }
                _ => result.push(c),
            },
            State::MaybeSlash => match c {
                '/' => state = State::LineComment,
                '*' => state = State::BlockComment,
                _ => {
                    // Not a comment start. Push the '/' we held back.
                    result.push('/');
                    // Now process the current character 'c' as if we were in Normal state.
                    match c {
                        '"' => {
                            state = State::StringLiteral;
                            result.push(c);
                        }
                        '\'' => {
                            state = State::CharLiteral;
                            result.push(c);
                        }
                        '/' => {
                            // We have '/ /'. Push the second '/' and stay in MaybeSlash.
                            // This case handles things like 'a / / b' correctly.
                            result.push(c);
                            // state remains MaybeSlash implicitly because the outer loop will process the next char
                        }
                        _ => {
                            result.push(c);
                            state = State::Normal; // Back to normal
                        }
                    }
                }
            },
            State::LineComment => {
                if c == '\n' {
                    result.push(c); // Keep the newline
                    state = State::Normal;
                }
                // Otherwise, consume the character (it's part of the comment)
            }
            State::BlockComment => {
                if c == '*' {
                    state = State::MaybeEndBlockComment;
                }
                // Consume the character
            }
            State::MaybeEndBlockComment => match c {
                '/' => state = State::Normal,
                '*' => {} // Still in MaybeEndBlockComment (e.g., /* ***/ )
                _ => state = State::BlockComment, // Not the end, back to BlockComment
            },
            State::StringLiteral => {
                result.push(c);
                match c {
                    '"' => state = State::Normal,
                    '\\' => state = State::StringEscape,
                    _ => {}
                }
            }
            State::StringEscape => {
                result.push(c); // Keep the escaped character
                state = State::StringLiteral; // Back to string state
            }
            State::CharLiteral => {
                result.push(c);
                match c {
                    '\'' => state = State::Normal,
                    '\\' => state = State::CharEscape,
                    _ => {} // Note: Multi-char literals like 'ab' are technically compiler errors, but we handle them gracefully
                }
            }
            State::CharEscape => {
                result.push(c); // Keep the escaped character
                state = State::CharLiteral; // Back to char state
            }
        }
    }

    // Handle edge case: input ends with '/'
    if matches!(state, State::MaybeSlash) {
        result.push('/');
    }

    // Handle edge case: input ends mid-block comment or mid-line comment
    // The loop finishes, and the remaining comment content is simply not added to `result`, which is correct.

    // Process the result line by line to remove trailing whitespace, then trim the whole result.
    let processed_result = result // Use the result from the state machine
        .lines()
        .map(|line| line.trim_end()) // Trim trailing whitespace from each line
        .collect::<Vec<&str>>()
        .join("\n") // Join back with newline separators
        .trim() // Trim leading/trailing whitespace (including newlines) from the final string
        .to_string(); // Convert back to String

    debug!(
        "Comment removal applied. Original len: {}, New len: {}",
        content.len(),
        processed_result.len() // Use processed length for logging
    );
    processed_result // Return the final trimmed string
}

/// Removes lines containing only whitespace.
///
/// # Examples
/// ```
/// use dircat::processing::filters::remove_empty_lines;
///
/// let text = "Line 1\n\n  \t  \nLine 4";
/// let expected = "Line 1\nLine 4";
///
/// assert_eq!(remove_empty_lines(text), expected);
/// ```
pub fn remove_empty_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<&str>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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
