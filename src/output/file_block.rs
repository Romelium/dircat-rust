//! Handles the formatting of a single file's content into a Markdown block.

use crate::constants::DEFAULT_LINE_NUMBER_WIDTH;
use crate::core_types::FileInfo;
use crate::output::formatter::format_path_for_display;
use crate::output::OutputConfig;
use anyhow::Result;
use log::debug; // Import debug
use std::io::Write;
use std::path::PathBuf;

/// Writes a single file's header and content block to the writer.
///
/// This function generates a Markdown header (`## File: ...`) followed by a
/// fenced code block containing the file's processed content.
///
/// # Arguments
/// * `writer` - The `Write` trait object to write the output to.
/// * `file_info` - A reference to the `FileInfo` struct for the file to be written.
/// * `config` - The application configuration, used for formatting options.
///
/// # Errors
/// Returns an error if any write operation fails.
///
/// # Examples
/// ```
/// use dircat::output::file_block::write_file_block;
/// use dircat::OutputConfig;
/// use dircat::core_types::FileInfo;
/// use std::path::PathBuf;
///
/// let opts = OutputConfig { backticks: false, filename_only_header: false, line_numbers: true, num_ticks: 3, summary: false, counts: false };
///
/// let file_info = FileInfo {
///     absolute_path: PathBuf::from("/tmp/test.rs"),
///     relative_path: PathBuf::from("src/test.rs"),
///     size: 12,
///     processed_content: Some("fn main() {\n}".to_string()),
///     counts: None,
///     is_process_last: false,
///     process_last_order: None,
///     is_binary: false,
/// };
///
/// let mut buffer = Vec::new();
/// write_file_block(&mut buffer, &file_info, &opts).unwrap();
///
/// let output = String::from_utf8(buffer).unwrap();
/// assert!(output.contains("## File: src/test.rs"));
/// assert!(output.contains("```rs"));
/// assert!(output.contains("    1 | fn main() {"));
/// assert!(output.contains("    2 | }"));
/// ```
pub fn write_file_block(
    writer: &mut dyn Write,
    file_info: &FileInfo,
    opts: &OutputConfig,
) -> Result<()> {
    let path_to_display = if opts.filename_only_header {
        file_info
            .relative_path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| file_info.relative_path.clone()) // Fallback if no filename
    } else {
        file_info.relative_path.clone()
    };
    // Persistent Debug Log: Log the path being used for the header
    debug!(
        "Path used for header display: '{}'",
        path_to_display.display()
    );
    let header_path_str = format_path_for_display(&path_to_display, opts);

    // --- Write File Header ---
    writeln!(writer, "## File: {}", header_path_str)?;

    // --- Write Code Block ---
    let extension_hint = file_info
        .relative_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or(""); // Default to empty hint if no extension

    let fence = "`".repeat(opts.num_ticks as usize);
    writeln!(writer, "{}{}", fence, extension_hint)?;

    // Write content line by line, adding line numbers if requested
    if let Some(content) = &file_info.processed_content {
        let lines = content.lines().collect::<Vec<_>>();
        let num_width = calculate_line_number_width(lines.len(), opts);

        if lines.is_empty() && content.is_empty() {
            // Handle empty file: write nothing between ``` blocks
        } else if lines.is_empty() && !content.is_empty() {
            // Handle file with content but no newline (single line)
            if opts.line_numbers {
                write!(writer, "{:>width$} | ", 1, width = num_width)?;
            }
            writeln!(writer, "{}", content)?;
        } else {
            // Handle multiple lines (or single line ending in newline)
            for (i, line) in lines.iter().enumerate() {
                if opts.line_numbers {
                    // Format with dynamic padding: "{line_num:>width$} | {line_content}"
                    write!(writer, "{:>width$} | ", i + 1, width = num_width)?;
                }
                writeln!(writer, "{}", line)?;
            }
        }
    } else {
        // Should not happen if processing was successful and not dry run
        log::warn!(
            "Content not available for file: {}",
            file_info.absolute_path.display()
        );
        writeln!(writer, "// Content not available")?;
    }

    writeln!(writer, "{}", fence)?;

    Ok(())
}

/// Calculates the required width for line numbers based on the total number of lines.
fn calculate_line_number_width(line_count: usize, opts: &OutputConfig) -> usize {
    if !opts.line_numbers {
        return 0; // No width needed if line numbers are off
    }
    if line_count == 0 {
        1 // Need width 1 even for 0 lines (e.g., "1 | ")
    } else {
        // Calculate digits needed for the largest line number
        ((line_count as f64).log10().floor() as usize) + 1
    }
    .max(DEFAULT_LINE_NUMBER_WIDTH) // Ensure a minimum width for alignment
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn create_test_opts(line_numbers: bool, filename_only: bool, backticks: bool) -> OutputConfig {
        crate::output::tests::create_mock_output_config(
            backticks,
            filename_only,
            line_numbers,
            false,
        )
    }

    // Helper to create dummy FileInfo
    fn create_file_info(relative_path: &str, content: Option<&str>) -> FileInfo {
        FileInfo {
            absolute_path: PathBuf::from("/absolute/path/to").join(relative_path),
            relative_path: PathBuf::from(relative_path),
            size: content.map_or(0, |c| c.len() as u64),
            processed_content: content.map(String::from),
            counts: None,
            is_process_last: false,
            process_last_order: None,
            is_binary: false,
        }
    }

    #[test]
    fn test_calculate_line_number_width() {
        let opts_ln_on = create_test_opts(true, false, false);
        let opts_ln_off = create_test_opts(false, false, false);

        assert_eq!(
            calculate_line_number_width(0, &opts_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(
            calculate_line_number_width(9, &opts_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(
            calculate_line_number_width(10, &opts_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(calculate_line_number_width(99999, &opts_ln_on), 5); // Exact width
        assert_eq!(calculate_line_number_width(100000, &opts_ln_on), 6); // Exact width

        assert_eq!(calculate_line_number_width(0, &opts_ln_off), 0);
        assert_eq!(calculate_line_number_width(1000, &opts_ln_off), 0);
    }

    #[test]
    fn test_write_file_block_basic() -> Result<()> {
        let opts = create_test_opts(false, false, false);
        let file_info =
            create_file_info("src/main.rs", Some("fn main() {\n    println!(\"Hi\");\n}"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: src/main.rs\n```rs\nfn main() {\n    println!(\"Hi\");\n}\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_no_extension() -> Result<()> {
        let opts = create_test_opts(false, false, false);
        let file_info = create_file_info("Makefile", Some("all: build"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: Makefile\n```\nall: build\n```\n"; // No language hint
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers() -> Result<()> {
        let opts = create_test_opts(true, false, false); // Line numbers ON
        let file_info = create_file_info("script.sh", Some("#!/bin/bash\necho $1")); // 2 lines
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: script.sh\n```sh\n    1 | #!/bin/bash\n    2 | echo $1\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers_single_line_no_newline() -> Result<()> {
        let opts = create_test_opts(true, false, false); // Line numbers ON
        let file_info = create_file_info("single.txt", Some("Just one line")); // 1 line, no newline
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: single.txt\n```txt\n    1 | Just one line\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers_empty_file() -> Result<()> {
        let opts = create_test_opts(true, false, false); // Line numbers ON
        let file_info = create_file_info("empty.txt", Some("")); // Empty content
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: empty.txt\n```txt\n```\n"; // No lines printed
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_filename_only() -> Result<()> {
        let opts = create_test_opts(false, true, false); // Filename only ON
        let file_info = create_file_info("path/to/file.py", Some("print('Hello')"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: file.py\n```py\nprint('Hello')\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_backticks() -> Result<()> {
        let opts = create_test_opts(false, false, true); // Backticks ON
        let file_info = create_file_info("data/config.toml", Some("[section]"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: `data/config.toml`\n```toml\n[section]\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_all_options() -> Result<()> {
        let opts = create_test_opts(true, true, true); // All options ON
        let file_info =
            create_file_info("utils/helper.js", Some("function help(){\nreturn true;\n}")); // 3 lines
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: `helper.js`\n```js\n    1 | function help(){\n    2 | return true;\n    3 | }\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_no_content() -> Result<()> {
        let opts = create_test_opts(false, false, false);
        let file_info = create_file_info("no_content.txt", None); // Content is None
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: no_content.txt\n```txt\n// Content not available\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
