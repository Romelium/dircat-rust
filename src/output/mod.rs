// src/output/mod.rs

use crate::config::Config;
use crate::core_types::FileInfo;
use anyhow::Result;
use log::debug;
use std::io::Write;

pub mod dry_run;
pub mod file_block;
pub mod formatter;
pub mod header;
pub mod summary;
pub mod writer;

/// A struct holding only the configuration options relevant to output formatting.
#[derive(Debug, Clone, Copy)]
pub struct OutputOptions {
    pub filename_only_header: bool,
    pub line_numbers: bool,
    pub backticks: bool,
    pub num_ticks: u8,
    pub summary: bool,
    pub counts: bool,
}

impl From<&Config> for OutputOptions {
    fn from(config: &Config) -> Self {
        Self {
            filename_only_header: config.filename_only_header,
            line_numbers: config.line_numbers,
            backticks: config.backticks,
            num_ticks: config.num_ticks,
            summary: config.summary,
            counts: config.counts,
        }
    }
}

/// A trait for formatting the processed file data into a specific output format.
pub trait OutputFormatter {
    /// Formats the processed files into the final output.
    fn format(
        &self,
        files: &[FileInfo],
        opts: &OutputOptions,
        writer: &mut dyn Write,
    ) -> Result<()>;

    /// Formats the discovered files for a dry run.
    fn format_dry_run(
        &self,
        files: &[FileInfo],
        opts: &OutputOptions,
        writer: &mut dyn Write,
    ) -> Result<()>;
}

/// The default formatter that generates Markdown output.
pub struct MarkdownFormatter;

impl OutputFormatter for MarkdownFormatter {
    /// Orchestrates the generation of the final Markdown output.
    fn format(
        &self,
        files: &[FileInfo],
        opts: &OutputOptions,
        writer: &mut dyn Write,
    ) -> Result<()> {
        debug!("Starting Markdown output generation...");

        if files.is_empty() {
            // No files to process, do nothing.
            return Ok(());
        }

        header::write_global_header(writer)?;

        let all_files_iter = files.iter();

        let mut first_block = true;
        for file_info in all_files_iter {
            if !first_block {
                // Add a blank line separator between file blocks
                writeln!(writer)?;
            }
            file_block::write_file_block(writer, file_info, opts)?;
            first_block = false;
        }

        if opts.summary {
            if !first_block {
                writeln!(writer)?;
            }
            let all_processed_files: Vec<&FileInfo> = files.iter().collect();
            summary::write_summary(writer, &all_processed_files, opts)?;
        }

        debug!("Markdown output generation complete.");
        writer.flush()?; // Ensure all buffered data is written before finalizing
        Ok(())
    }

    fn format_dry_run(
        &self,
        files: &[FileInfo],
        opts: &OutputOptions,
        writer: &mut dyn Write,
    ) -> Result<()> {
        let file_refs: Vec<&FileInfo> = files.iter().collect();
        dry_run::write_dry_run_output(writer, &file_refs, opts)
    }
}

// Internal test module for output generation logic
#[cfg(test)]
pub(crate) mod tests {
    // Make module public within the crate for use by siblings
    use super::*;
    use crate::core_types::{FileCounts, FileInfo};
    use std::path::PathBuf;

    pub(crate) fn create_mock_config(
        backticks: bool,
        filename_only: bool,
        line_numbers: bool,
        summary: bool,
    ) -> OutputOptions {
        OutputOptions {
            backticks,
            filename_only_header: filename_only,
            line_numbers,
            summary,
            counts: false, // Default to false for these tests unless specified
            num_ticks: 3,
        }
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
            is_binary: false,
        }
    }

    #[test]
    fn test_markdown_formatter_basic() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
        let content_b = "Content B";
        let content_a = "Content A";
        let mut file1 = create_mock_file_info("b.txt", 10);
        file1.processed_content = Some(content_b.to_string());
        let mut file2 = create_mock_file_info("a.rs", 20);
        file2.processed_content = Some(content_a.to_string());
        let formatter = MarkdownFormatter;
        let normal_files = vec![file1, file2]; // Unsorted input
        let mut output = Vec::new();

        formatter.format(&normal_files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert exact output and order
        assert!(output_str.starts_with("## File: b.txt"));
        assert!(output_str.contains("```\n\n## File: a.rs"));
        assert!(!output_str.ends_with("\n\n"));
        assert!(output_str.ends_with("\n"));

        Ok(())
    }

    #[test]
    fn test_markdown_formatter_with_last_files() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
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

        // Manually sort the files to simulate the output of the `discover` function,
        // which is the data `generate_output` now expects.
        let mut normal_files = vec![file1, file2]; // c.txt, a.rs
        let mut last_files = vec![last1, last0]; // last1.md (order 0), last0.toml (order 1)

        normal_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path)); // Sorts to [a.rs, c.txt]
        last_files.sort_by_key(|f| f.process_last_order); // Sorts to [last1.md, last0.toml]

        let all_files: Vec<FileInfo> = normal_files.into_iter().chain(last_files).collect();
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format(&all_files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert order and separators
        assert!(output_str.starts_with("## File: a.rs"));
        assert!(output_str.contains("```\n\n## File: c.txt"));
        assert!(output_str.contains("```\n\n## File: last1.md"));
        assert!(output_str.contains("```\n\n## File: last0.toml"));

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
    fn test_markdown_formatter_with_summary() -> Result<()> {
        let mut opts = create_mock_config(false, false, false, true); // summary = true
        opts.counts = true; // Enable counts as well for completeness
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
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format(&normal_files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // Assert key components and separators
        assert!(output_str.starts_with("## File: b.txt"));
        assert!(output_str.contains("```\n\n## File: a.rs"));
        assert!(output_str.contains("```\n\n---\nProcessed Files: (2)\n")); // Summary header
        assert!(output_str.contains("- a.rs (L:1 C:9 W:2)\n")); // Summary items sorted
        assert!(output_str.contains("- b.txt (L:1 C:9 W:2)\n"));

        // Check summary is at the end
        assert!(output_str.ends_with("- b.txt (L:1 C:9 W:2)\n"));

        Ok(())
    }

    #[test]
    fn test_markdown_formatter_no_files() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
        let files = vec![];
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format(&files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // With no files, output should be empty
        assert_eq!(output_str, "");
        Ok(())
    }

    #[test]
    fn test_markdown_formatter_file_with_no_content() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
        let file = create_mock_file_info("empty.txt", 0); // processed_content is None
        let files = vec![file];
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format(&files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        let expected = "## File: empty.txt\n```txt\n// Content not available\n```\n";
        assert_eq!(output_str, expected);
        Ok(())
    }

    #[test]
    fn test_markdown_formatter_file_with_only_newline() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
        let mut file = create_mock_file_info("newline.txt", 1);
        file.processed_content = Some("\n".to_string());
        let files = vec![file];
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format(&files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        // The content has one line ("") and then the loop finishes.
        let expected = "## File: newline.txt\n```txt\n\n```\n";
        assert_eq!(output_str, expected);
        Ok(())
    }

    #[test]
    fn test_markdown_formatter_dry_run_no_files() -> Result<()> {
        let opts = create_mock_config(false, false, false, false);
        let files = vec![];
        let formatter = MarkdownFormatter;
        let mut output = Vec::new();

        formatter.format_dry_run(&files, &opts, &mut output)?;
        let output_str = String::from_utf8(output)?;

        let expected = "\n--- Dry Run: Files that would be processed ---\n--- End Dry Run ---\n";
        assert_eq!(output_str, expected);
        Ok(())
    }
}
