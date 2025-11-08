
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
