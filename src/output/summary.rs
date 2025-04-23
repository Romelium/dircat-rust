// src/output/summary.rs

use crate::config::Config;
use crate::constants;
use crate::core_types::FileInfo; // Removed unused FileCounts
use crate::output::formatter::format_path_for_display;
use anyhow::Result;
use log::debug;
use std::io::Write;

/// Writes the summary section (list of processed files, optionally with counts)
/// to the output writer. Assumes files are pre-sorted if necessary.
pub fn write_summary(
    writer: &mut dyn Write,
    files: &[&FileInfo], // Takes refs to avoid cloning
    config: &Config,
) -> Result<()> {
    debug!("Writing summary for {} files...", files.len());
    write!(writer, "\n{}\n", constants::SUMMARY_SEPARATOR)?;
    writeln!(
        writer,
        "{}: ({})",
        constants::SUMMARY_HEADER_PREFIX,
        files.len()
    )?;

    // Sort all files alphabetically by relative path for the summary list
    // Clone only the necessary info (relative path, counts) for sorting
    let mut summary_items: Vec<_> = files
        .iter()
        .map(|fi| (fi.relative_path.clone(), fi.counts)) // Clone PathBuf and copy Option<FileCounts>
        .collect();
    summary_items.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by relative_path

    for (rel_path, counts_opt) in summary_items {
        let path_str = format_path_for_display(&rel_path, config);
        if config.counts {
            if let Some(counts) = counts_opt {
                writeln!(
                    writer,
                    "- {} (L:{} C:{} W:{})",
                    path_str, counts.lines, counts.characters, counts.words
                )?;
            } else {
                // Should not happen if config.counts is true and processing succeeded,
                // but handle defensively.
                log::warn!("Counts requested but not available for: {}", path_str);
                writeln!(writer, "- {} (Counts not available)", path_str)?;
            }
        } else {
            writeln!(writer, "- {}", path_str)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, OutputDestination};
    use crate::core_types::{FileCounts, FileInfo};
    use std::io::Cursor;
    use std::path::PathBuf;

    // Helper to create a minimal Config for testing
    fn create_test_config(counts: bool, backticks: bool) -> Config {
        Config {
            input_path: PathBuf::from("/base"), // Mock base path
            base_path_display: "/base".to_string(),
            input_is_file: false, // <-- ADDED: Default to false for tests
            counts,
            backticks,
            summary: true, // Summary must be true for these tests
            // Other fields defaulted
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            path_regex: None,
            filename_regex: None,
            use_gitignore: true,
            remove_comments: false,
            remove_empty_lines: false,
            filename_only_header: false,
            line_numbers: false,
            output_destination: OutputDestination::Stdout,
            process_last: None,
            only_last: false,
            dry_run: false,
        }
    }

    // Helper to create dummy FileInfo
    fn create_file_info(relative_path: &str, counts: Option<FileCounts>) -> FileInfo {
        FileInfo {
            absolute_path: PathBuf::from("/absolute/path/to").join(relative_path),
            relative_path: PathBuf::from(relative_path),
            size: 100,
            processed_content: Some("dummy content".to_string()), // Need some content for processing logic
            counts,
            is_process_last: false,
            process_last_order: None,
        }
    }

    #[test]
    fn test_summary_empty() -> Result<()> {
        let config = create_test_config(false, false);
        let files = vec![];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (0)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_no_counts() -> Result<()> {
        let config = create_test_config(false, false);
        let fi1 = create_file_info("z_file.txt", None);
        let fi2 = create_file_info("a_file.rs", None);
        let fi3 = create_file_info("sub/b_file.md", None);
        let files = vec![&fi1, &fi2, &fi3]; // Unsorted input refs
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (3)\n- a_file.rs\n- sub/b_file.md\n- z_file.txt\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts() -> Result<()> {
        let config = create_test_config(true, false); // Counts ON
        let counts1 = Some(FileCounts {
            lines: 10,
            characters: 100,
            words: 20,
        });
        let counts2 = Some(FileCounts {
            lines: 1,
            characters: 5,
            words: 1,
        });
        let fi1 = create_file_info("z_file.txt", counts1);
        let fi2 = create_file_info("a_file.rs", counts2);
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (2)\n- a_file.rs (L:1 C:5 W:1)\n- z_file.txt (L:10 C:100 W:20)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts_missing() -> Result<()> {
        // Test the defensive handling when counts are requested but missing on a FileInfo
        let config = create_test_config(true, false); // Counts ON
        let counts1 = Some(FileCounts {
            lines: 10,
            characters: 100,
            words: 20,
        });
        let fi1 = create_file_info("z_file.txt", counts1);
        let fi2 = create_file_info("a_file.rs", None); // Counts missing
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (2)\n- a_file.rs (Counts not available)\n- z_file.txt (L:10 C:100 W:20)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_backticks() -> Result<()> {
        let config = create_test_config(false, true); // Backticks ON
        let fi1 = create_file_info("file with space.txt", None);
        let fi2 = create_file_info("another.rs", None);
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (2)\n- `another.rs`\n- `file with space.txt`\n"; // Paths are backticked and sorted
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts_and_backticks() -> Result<()> {
        let config = create_test_config(true, true); // Counts and Backticks ON
        let counts1 = Some(FileCounts {
            lines: 2,
            characters: 15,
            words: 3,
        });
        let fi1 = create_file_info("data.csv", counts1);
        let files = vec![&fi1];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &config)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n---\nProcessed Files: (1)\n- `data.csv` (L:2 C:15 W:3)\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
