use log::debug;

/// Removes C/C++ style comments (// and /* ... */) using a state machine.
///
/// This function correctly handles comments within string and character literals,
/// as well as escaped characters. After removing comments, it performs two cleanup steps:
/// 1. Trims trailing whitespace from each resulting line.
/// 2. Trims leading and trailing whitespace (including newlines) from the entire final string.
///
/// This ensures the output is clean and free of artifacts from removed comments.
///
/// # Examples
/// ```
/// use dircat::processing::filters::remove_comments;
/// let code = r#"
///     let x = 1; // A comment
///     /* Another comment */
///     let y = 2;
/// "#;
///
/// let expected = "    let x = 1;\n\n    let y = 2;";
///
/// assert_eq!(remove_comments(code), expected);
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
        .trim_matches(|c: char| c == '\r' || c == '\n') // Trim leading/trailing newlines only, preserving indentation.
        .to_string(); // Convert back to String

    debug!(
        "Comment removal applied. Original len: {}, New len: {}",
        content.len(),
        processed_result.len() // Use processed length for logging
    );
    processed_result // Return the final trimmed string
}
