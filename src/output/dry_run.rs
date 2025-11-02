// src/output/dry_run.rs

use crate::config::Config;
use crate::core_types::FileInfo;
use crate::output::formatter::format_path_for_display;
use anyhow::Result;
use log::debug;
use std::io::Write;

/// Writes the output for a dry run (-D).
/// Lists the files that would be processed, sorted alphabetically.
pub(crate) fn write_dry_run_output(
    writer: &mut dyn Write,
    files: &[&FileInfo], // Takes refs to avoid cloning full FileInfo
    config: &Config,
) -> Result<()> {
    debug!("Executing dry run output...");
    writeln!(writer, "\n--- Dry Run: Files that would be processed ---")?;

    // Clone only the necessary info (relative path) for sorting
    let mut paths_to_print: Vec<_> = files
        .iter()
        .map(|fi| fi.relative_path.clone()) // Clone PathBuf
        .collect();

    // Sort alphabetically by relative path for dry run display
    paths_to_print.sort();

    for rel_path in paths_to_print {
        let path_str = format_path_for_display(&rel_path, config);
        writeln!(writer, "- {}", path_str)?;
    }

    writeln!(writer, "--- End Dry Run ---")?;
    writer.flush()?; // Flush dry run output
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core_types::FileInfo;
    use std::io::Cursor;
    use std::path::PathBuf;

    // Helper to create a minimal Config for testing
    fn create_test_config(backticks: bool) -> Config {
        let mut config = Config::new_for_test();
        config.input_path = PathBuf::from("/base");
        config.base_path_display = "/base".to_string();
        config.backticks = backticks;
        config.dry_run = true; // Assume dry run for these tests
        config
    }

    // Helper to create dummy FileInfo
    fn create_file_info(relative_path: &str) -> FileInfo {
        FileInfo {
            absolute_path: PathBuf::from("/absolute/path/to").join(relative_path),
            relative_path: PathBuf::from(relative_path),
            size: 100,
            processed_content: None,
            counts: None,
            is_process_last: false,
            process_last_order: None,
            is_binary: false,
        }
    }

    #[test]
    fn test_dry_run_output_empty() -> Result<()> {
        let config = create_test_config(false);
        let files = vec![];
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n--- Dry Run: Files that would be processed ---\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_dry_run_output_sorted() -> Result<()> {
        let config = create_test_config(false);
        let fi1 = create_file_info("z_file.txt");
        let fi2 = create_file_info("a_file.rs");
        let fi3 = create_file_info("sub/b_file.md");
        let files = vec![&fi1, &fi2, &fi3]; // Unsorted input refs
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n--- Dry Run: Files that would be processed ---\n- a_file.rs\n- sub/b_file.md\n- z_file.txt\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_dry_run_output_with_backticks() -> Result<()> {
        let config = create_test_config(true); // Enable backticks
        let fi1 = create_file_info("file.txt");
        let files = vec![&fi1];
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected =
            "\n--- Dry Run: Files that would be processed ---\n- `file.txt`\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
