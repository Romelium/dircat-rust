// src/output/mod.rs

use crate::config::Config;
use crate::core_types::FileInfo; // Removed unused FileCounts
use anyhow::Result;
use log::debug;
use std::io::Write;

pub mod dry_run;
pub mod file_block;
pub mod formatter;
pub mod header; // <-- Now points to the created file
pub mod summary;
pub mod writer; // Manages the output destination

/// Orchestrates the generation of the final output string or file content.
/// This function decides whether to perform a dry run or generate the full output.
/// It does NOT handle setting up the writer or finalizing (like clipboard copy).
pub fn generate_output(
    normal_files: &[FileInfo],
    last_files: &[FileInfo],
    config: &Config,
    writer: &mut dyn Write, // Takes a mutable ref to the already setup writer
) -> Result<()> {
    debug!("Starting output generation...");

    // Write Global Header (always, unless dry run handled it)
    header::write_global_header(writer)?; // <-- This call should now resolve

    // --- Write Normal Files ---
    // Sort normal files alphabetically by relative path
    let mut sorted_normal_files = normal_files.to_vec(); // Clone to sort
    sorted_normal_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    for file_info in &sorted_normal_files {
        file_block::write_file_block(writer, file_info, config)?;
    }

    // --- Write Last Files ---
    // These are already sorted by the order specified in -z during discovery
    for file_info in last_files {
        file_block::write_file_block(writer, file_info, config)?;
    }

    // --- Write Summary ---
    if config.summary {
        let all_processed_files: Vec<&FileInfo> = sorted_normal_files
            .iter()
            .chain(last_files.iter())
            .collect();
        summary::write_summary(writer, &all_processed_files, config)?;
    }

    debug!("Output generation complete.");
    writer.flush()?; // Ensure all buffered data is written before finalizing
    Ok(())
}

// Internal test module for output generation logic
#[cfg(test)]
pub(crate) mod tests {
    // Make module public within the crate for use by siblings
    use super::*;
    use crate::config::Config; // Keep Config, OutputDestination
    use crate::constants;
    use crate::core_types::{FileCounts, FileInfo};
    use std::path::PathBuf;

    pub(crate) fn create_mock_config(
        backticks: bool,
        filename_only: bool,
        line_numbers: bool,
        summary: bool,
    ) -> Config {
        let mut config = Config::new_for_test();
        config.input_path = PathBuf::from("/base");
        config.base_path_display = "/base".to_string();
        config.backticks = backticks;
        config.filename_only_header = filename_only;
        config.line_numbers = line_numbers;
        config.summary = summary; // Also controls if counts are checked if enabled later
        config
    }

    pub(crate) fn create_mock_file_info(relative_path: &str, size: u64) -> FileInfo {
        let abs_path = PathBuf::from("/base").join(relative_path);
        FileInfo {
            absolute_path: abs_path,
            relative_path: PathBuf::from(relative_path),
            size,
            processed_content: None, // Default to None, set by test if needed
            counts: None,            // Default to None
            is_process_last: false,
            process_last_order: None,
            is_binary: false, // <-- ADDED default
        }
    }

    #[test]
    fn test_generate_output_basic() -> Result<()> {
        let config = create_mock_config(false, false, false, false);
        let content_b = "Content B";
        let content_a = "Content A";
        let mut file1 = create_mock_file_info("b.txt", 10);
        file1.processed_content = Some(content_b.to_string());
        let mut file2 = create_mock_file_info("a.rs", 20);
        file2.processed_content = Some(content_a.to_string());
        let normal_files = vec![file1, file2]; // Unsorted input
        let last_files = vec![];
        let mut output = Vec::new();

        generate_output(&normal_files, &last_files, &config, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert key components and order
        assert!(output_str.starts_with(constants::OUTPUT_FILE_HEADER));
        assert!(output_str.contains("\n## File: a.rs\n```rs\nContent A\n```\n")); // Check first file block (sorted)
        assert!(output_str.contains("\n## File: b.txt\n```txt\nContent B\n```\n")); // Check second file block (sorted)

        // Check relative order
        let pos_a = output_str.find("## File: a.rs").unwrap();
        let pos_b = output_str.find("## File: b.txt").unwrap();
        assert!(pos_a < pos_b);

        Ok(())
    }

    #[test]
    fn test_generate_output_with_last_files() -> Result<()> {
        let config = create_mock_config(false, false, false, false);
        let content_c = "Content C";
        let content_a = "Content A";
        let content_last1 = "Last 1";
        let content_last0 = "Last 0"; // Renamed to avoid confusion with order

        let mut file1 = create_mock_file_info("c.txt", 10);
        file1.processed_content = Some(content_c.to_string());
        let mut file2 = create_mock_file_info("a.rs", 20);
        file2.processed_content = Some(content_a.to_string());
        let mut last1 = create_mock_file_info("last1.md", 30); // Matches first -z pattern
        last1.processed_content = Some(content_last1.to_string());
        last1.is_process_last = true;
        last1.process_last_order = Some(0);
        let mut last0 = create_mock_file_info("last0.toml", 40); // Matches second -z pattern
        last0.processed_content = Some(content_last0.to_string());
        last0.is_process_last = true;
        last0.process_last_order = Some(1); // Simulate order given by -z

        let normal_files = vec![file1, file2]; // Unsorted normal
        let last_files = vec![last1, last0]; // Order here matches discovery sort key
        let mut output = Vec::new();

        generate_output(&normal_files, &last_files, &config, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert key components and order
        assert!(output_str.starts_with(constants::OUTPUT_FILE_HEADER));
        assert!(output_str.contains("\n## File: a.rs\n```rs\nContent A\n```\n")); // Normal sorted
        assert!(output_str.contains("\n## File: c.txt\n```txt\nContent C\n```\n")); // Normal sorted
        assert!(output_str.contains("\n## File: last1.md\n```md\nLast 1\n```\n")); // First last file
        assert!(output_str.contains("\n## File: last0.toml\n```toml\nLast 0\n```\n")); // Second last file

        // Check relative order
        let pos_a = output_str.find("## File: a.rs").unwrap();
        let pos_c = output_str.find("## File: c.txt").unwrap();
        let pos_last1 = output_str.find("## File: last1.md").unwrap();
        let pos_last0 = output_str.find("## File: last0.toml").unwrap();

        assert!(pos_a < pos_c); // Normal files sorted
        assert!(pos_c < pos_last1); // Normal files before last files
        assert!(pos_last1 < pos_last0); // Last files in specified order

        Ok(())
    }

    #[test]
    fn test_generate_output_with_summary() -> Result<()> {
        let mut config = create_mock_config(false, false, false, true); // summary = true
        config.counts = true; // Enable counts as well for completeness
        let content_b = "Content B";
        let content_a = "Content A";

        let mut file1 = create_mock_file_info("b.txt", 10);
        file1.processed_content = Some(content_b.to_string());
        file1.counts = Some(FileCounts {
            lines: 1,
            characters: 9,
            words: 2,
        });
        let mut file2 = create_mock_file_info("a.rs", 20);
        file2.processed_content = Some(content_a.to_string());
        file2.counts = Some(FileCounts {
            lines: 1,
            characters: 9,
            words: 2,
        });

        let normal_files = vec![file1, file2];
        let last_files = vec![];
        let mut output = Vec::new();

        generate_output(&normal_files, &last_files, &config, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert key components
        assert!(output_str.starts_with(constants::OUTPUT_FILE_HEADER));
        assert!(output_str.contains("\n## File: a.rs\n```rs\nContent A\n```\n"));
        assert!(output_str.contains("\n## File: b.txt\n```txt\nContent B\n```\n"));
        assert!(output_str.contains("\n---\nProcessed Files: (2)\n")); // Summary header
        assert!(output_str.contains("- a.rs (L:1 C:9 W:2)\n")); // Summary items sorted
        assert!(output_str.contains("- b.txt (L:1 C:9 W:2)\n"));

        // Check summary is at the end
        assert!(output_str.ends_with("- b.txt (L:1 C:9 W:2)\n"));

        Ok(())
    }

    #[test]
    fn test_generate_output_no_files() -> Result<()> {
        let config = create_mock_config(false, false, false, false);
        let normal_files = vec![];
        let last_files = vec![];
        let mut output = Vec::new();

        generate_output(&normal_files, &last_files, &config, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Only global header should be present, followed by the extra newline
        let expected = format!("{}\n\n", constants::OUTPUT_FILE_HEADER.trim_end());
        assert_eq!(output_str, expected);
        Ok(())
    }
}
