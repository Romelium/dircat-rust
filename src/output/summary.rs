// src/output/summary.rs

use crate::constants;
use crate::core_types::FileInfo;
use crate::output::formatter::format_path_for_display;
use crate::output::OutputConfig;
use anyhow::Result;
use log::debug;
use std::io::Write;

/// Writes the summary section (list of processed files, optionally with counts)
/// to the output writer. Assumes files are pre-sorted if necessary.
pub fn write_summary(
    writer: &mut dyn Write,
    files: &[&FileInfo], // Takes refs to avoid cloning
    opts: &OutputConfig,
) -> Result<()> {
    debug!("Writing summary for {} files...", files.len());
    writeln!(writer, "{}", constants::SUMMARY_SEPARATOR)?;
    writeln!(
        writer,
        "{}: ({})",
        constants::SUMMARY_HEADER_PREFIX,
        files.len()
    )?;

    // Sort the slice of references directly to avoid cloning PathBufs.
    let mut sorted_files = files.to_vec();
    sorted_files.sort_by_key(|fi| &fi.relative_path);

    for file_info in sorted_files {
        let path_str = format_path_for_display(&file_info.relative_path, opts);
        if opts.counts {
            if let Some(counts) = file_info.counts {
                if file_info.is_binary {
                    // Special format for binary files in counts summary
                    writeln!(writer, "- {} (Binary C:{})", path_str, counts.characters)?;
                } else {
                    writeln!(
                        writer,
                        "- {} (L:{} C:{} W:{})",
                        path_str, counts.lines, counts.characters, counts.words
                    )?;
                }
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
    use crate::core_types::{FileCounts, FileInfo};
    use std::io::Cursor;
    use std::path::PathBuf;

    // Helper to create a minimal Config for testing
    fn create_test_opts(counts: bool, backticks: bool) -> OutputConfig {
        let mut opts =
            crate::output::tests::create_mock_output_config(backticks, false, false, true);
        opts.counts = counts;
        opts.summary = true; // Summary must be true for these tests
        opts
    }

    // Helper to create dummy FileInfo
    fn create_file_info(
        relative_path: &str,
        counts: Option<FileCounts>,
        is_binary: bool,
    ) -> FileInfo {
        FileInfo {
            absolute_path: PathBuf::from("/absolute/path/to").join(relative_path),
            relative_path: PathBuf::from(relative_path),
            size: 100,
            processed_content: Some("dummy content".to_string()), // Need some content for processing logic
            counts,
            is_process_last: false,
            process_last_order: None,
            is_binary, // Set binary flag
        }
    }

    #[test]
    fn test_summary_empty() -> Result<()> {
        let opts = create_test_opts(false, false);
        let files = vec![];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (0)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_no_counts() -> Result<()> {
        let opts = create_test_opts(false, false);
        let fi1 = create_file_info("z_file.txt", None, false);
        let fi2 = create_file_info("a_file.rs", None, false);
        let fi3 = create_file_info("sub/b_file.md", None, false);
        let files = vec![&fi1, &fi2, &fi3]; // Unsorted input refs
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (3)\n- a_file.rs\n- sub/b_file.md\n- z_file.txt\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts() -> Result<()> {
        let opts = create_test_opts(true, false); // Counts ON
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
        let fi1 = create_file_info("z_file.txt", counts1, false);
        let fi2 = create_file_info("a_file.rs", counts2, false);
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (2)\n- a_file.rs (L:1 C:5 W:1)\n- z_file.txt (L:10 C:100 W:20)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts_missing() -> Result<()> {
        // Test the defensive handling when counts are requested but missing on a FileInfo
        let opts = create_test_opts(true, false); // Counts ON
        let counts1 = Some(FileCounts {
            lines: 10,
            characters: 100,
            words: 20,
        });
        let fi1 = create_file_info("z_file.txt", counts1, false);
        let fi2 = create_file_info("a_file.rs", None, false); // Counts missing
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (2)\n- a_file.rs (Counts not available)\n- z_file.txt (L:10 C:100 W:20)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_backticks() -> Result<()> {
        let opts = create_test_opts(false, true); // Backticks ON
        let fi1 = create_file_info("file with space.txt", None, false);
        let fi2 = create_file_info("another.rs", None, false);
        let files = vec![&fi1, &fi2];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (2)\n- `another.rs`\n- `file with space.txt`\n"; // Paths are backticked and sorted
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_counts_and_backticks() -> Result<()> {
        let opts = create_test_opts(true, true); // Counts and Backticks ON
        let counts1 = Some(FileCounts {
            lines: 2,
            characters: 15,
            words: 3,
        });
        let fi1 = create_file_info("data.csv", counts1, false);
        let files = vec![&fi1];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "---\nProcessed Files: (1)\n- `data.csv` (L:2 C:15 W:3)\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_summary_with_binary_counts() -> Result<()> {
        let opts = create_test_opts(true, false); // Counts ON
        let counts_text = Some(FileCounts {
            lines: 10,
            characters: 100,
            words: 20,
        });
        let counts_binary = Some(FileCounts {
            lines: 0, // Lines/words are 0 for binary
            characters: 256,
            words: 0,
        });
        let fi_text = create_file_info("text.txt", counts_text, false);
        let fi_binary = create_file_info("binary.bin", counts_binary, true); // Mark as binary
        let files = vec![&fi_text, &fi_binary];
        let mut writer = Cursor::new(Vec::new());
        write_summary(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Expect different format for binary file count
        let expected = "---\nProcessed Files: (2)\n- binary.bin (Binary C:256)\n- text.txt (L:10 C:100 W:20)\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
