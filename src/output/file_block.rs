use crate::config::Config;
use crate::constants::DEFAULT_LINE_NUMBER_WIDTH;
use crate::core_types::FileInfo;
use crate::output::formatter::format_path_for_display;
use anyhow::Result;
use log::debug; // Import debug
use std::io::Write;
use std::path::PathBuf;

/// Writes a single file's header and content block to the writer.
pub fn write_file_block(
    writer: &mut dyn Write,
    file_info: &FileInfo,
    config: &Config,
) -> Result<()> {
    let path_to_display = if config.filename_only_header {
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
    let header_path_str = format_path_for_display(&path_to_display, config);

    // --- Write File Header ---
    writeln!(writer, "## File: {}", header_path_str)?;

    // --- Write Code Block ---
    let extension_hint = file_info
        .relative_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or(""); // Default to empty hint if no extension

    writeln!(writer, "```{}", extension_hint)?;

    // Write content line by line, adding line numbers if requested
    if let Some(content) = &file_info.processed_content {
        let lines = content.lines().collect::<Vec<_>>();
        let num_width = calculate_line_number_width(lines.len(), config);

        if lines.is_empty() && content.is_empty() {
            // Handle empty file: write nothing between ``` blocks
        } else if lines.is_empty() && !content.is_empty() {
            // Handle file with content but no newline (single line)
            if config.line_numbers {
                write!(writer, "{:>width$} | ", 1, width = num_width)?;
            }
            writeln!(writer, "{}", content)?;
        } else {
            // Handle multiple lines (or single line ending in newline)
            for (i, line) in lines.iter().enumerate() {
                if config.line_numbers {
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

    writeln!(writer, "```")?;

    Ok(())
}

/// Calculates the required width for line numbers based on the total number of lines.
fn calculate_line_number_width(line_count: usize, config: &Config) -> usize {
    if !config.line_numbers {
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
    use crate::config::Config;
    use std::io::Cursor;
    use std::path::PathBuf;

    // Mock Config for testing purposes - using a helper function now
    fn create_test_config(line_numbers: bool, filename_only: bool, backticks: bool) -> Config {
        let mut config = Config::new_for_test();
        config.input_path = PathBuf::from("/base");
        config.base_path_display = "/base".to_string();
        config.line_numbers = line_numbers;
        config.filename_only_header = filename_only;
        config.backticks = backticks;
        config
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
        let config_ln_on = create_test_config(true, false, false);
        let config_ln_off = create_test_config(false, false, false);

        assert_eq!(
            calculate_line_number_width(0, &config_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(
            calculate_line_number_width(9, &config_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(
            calculate_line_number_width(10, &config_ln_on),
            DEFAULT_LINE_NUMBER_WIDTH
        ); // Min width
        assert_eq!(calculate_line_number_width(99999, &config_ln_on), 5); // Exact width
        assert_eq!(calculate_line_number_width(100000, &config_ln_on), 6); // Exact width

        assert_eq!(calculate_line_number_width(0, &config_ln_off), 0);
        assert_eq!(calculate_line_number_width(1000, &config_ln_off), 0);
    }

    #[test]
    fn test_write_file_block_basic() -> Result<()> {
        let config = create_test_config(false, false, false);
        let file_info =
            create_file_info("src/main.rs", Some("fn main() {\n    println!(\"Hi\");\n}"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: src/main.rs\n```rs\nfn main() {\n    println!(\"Hi\");\n}\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_no_extension() -> Result<()> {
        let config = create_test_config(false, false, false);
        let file_info = create_file_info("Makefile", Some("all: build"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: Makefile\n```\nall: build\n```\n"; // No language hint
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers() -> Result<()> {
        let config = create_test_config(true, false, false); // Line numbers ON
        let file_info = create_file_info("script.sh", Some("#!/bin/bash\necho $1")); // 2 lines
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: script.sh\n```sh\n    1 | #!/bin/bash\n    2 | echo $1\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers_single_line_no_newline() -> Result<()> {
        let config = create_test_config(true, false, false); // Line numbers ON
        let file_info = create_file_info("single.txt", Some("Just one line")); // 1 line, no newline
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: single.txt\n```txt\n    1 | Just one line\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_line_numbers_empty_file() -> Result<()> {
        let config = create_test_config(true, false, false); // Line numbers ON
        let file_info = create_file_info("empty.txt", Some("")); // Empty content
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: empty.txt\n```txt\n```\n"; // No lines printed
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_filename_only() -> Result<()> {
        let config = create_test_config(false, true, false); // Filename only ON
        let file_info = create_file_info("path/to/file.py", Some("print('Hello')"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: file.py\n```py\nprint('Hello')\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_backticks() -> Result<()> {
        let config = create_test_config(false, false, true); // Backticks ON
        let file_info = create_file_info("data/config.toml", Some("[section]"));
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: `data/config.toml`\n```toml\n[section]\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_all_options() -> Result<()> {
        let config = create_test_config(true, true, true); // All options ON
        let file_info =
            create_file_info("utils/helper.js", Some("function help(){\nreturn true;\n}")); // 3 lines
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Assumes DEFAULT_LINE_NUMBER_WIDTH is 5
        let expected = "## File: `helper.js`\n```js\n    1 | function help(){\n    2 | return true;\n    3 | }\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_write_file_block_no_content() -> Result<()> {
        let config = create_test_config(false, false, false);
        let file_info = create_file_info("no_content.txt", None); // Content is None
        let mut writer = Cursor::new(Vec::new());
        write_file_block(&mut writer, &file_info, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "## File: no_content.txt\n```txt\n// Content not available\n```\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
