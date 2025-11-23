// src/output/dry_run.rs

use crate::core_types::FileInfo;
use crate::output::formatter::format_path_for_display;
use crate::output::OutputConfig;
use anyhow::Result;
use log::debug;
use std::io::Write;

/// Writes the output for a dry run (-D).
///
/// This function lists the relative paths of files that would be processed.
/// It iterates over the provided files slice in order, without re-sorting.
/// The caller is responsible for ensuring the files are in the desired display order.
#[doc(hidden)] // This is a public helper but not intended for direct library use.
pub(crate) fn write_dry_run_output(
    writer: &mut dyn Write,
    files: &[&FileInfo], // Takes refs to avoid cloning full FileInfo
    opts: &OutputConfig,
) -> Result<()> {
    debug!("Executing dry run output...");
    writeln!(writer, "\n--- Dry Run: Files that would be processed ---")?;

    // Iterate directly over the provided slice to preserve order.
    for file_info in files {
        let path_str = format_path_for_display(&file_info.relative_path, opts);
        writeln!(writer, "- {}", path_str)?;
    }

    writeln!(writer, "--- End Dry Run ---")?;
    writer.flush()?; // Flush dry run output
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_types::FileInfo;
    use std::io::Cursor;
    use std::path::PathBuf;

    // Helper to create a minimal Config for testing
    fn create_test_opts(backticks: bool) -> OutputConfig {
        OutputConfig {
            backticks,
            ..crate::output::tests::create_mock_output_config(false, false, false, false)
        }
    }

    // Helper to create dummy FileInfo
    fn create_file_info(relative_path: &str) -> FileInfo {
        FileInfo {
            absolute_path: PathBuf::from("/absolute/path/to").join(relative_path),
            relative_path: PathBuf::from(relative_path),
            size: 100,
            ..Default::default()
        }
    }

    #[test]
    fn test_dry_run_output_empty() -> Result<()> {
        let opts = create_test_opts(false);
        let files = vec![];
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected = "\n--- Dry Run: Files that would be processed ---\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_dry_run_output_preserves_order() -> Result<()> {
        let opts = create_test_opts(false);
        let fi1 = create_file_info("z_file.txt");
        let fi2 = create_file_info("a_file.rs");
        let fi3 = create_file_info("sub/b_file.md");
        
        // Pass files in a specific non-alphabetical order
        let files = vec![&fi1, &fi2, &fi3]; 
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        // Expect the output to match the input order exactly
        let expected = "\n--- Dry Run: Files that would be processed ---\n- z_file.txt\n- a_file.rs\n- sub/b_file.md\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }

    #[test]
    fn test_dry_run_output_with_backticks() -> Result<()> {
        let opts = create_test_opts(true); // Enable backticks
        let fi1 = create_file_info("file.txt");
        let files = vec![&fi1];
        let mut writer = Cursor::new(Vec::new());
        write_dry_run_output(&mut writer, &files, &opts)?;

        let output = String::from_utf8(writer.into_inner())?;
        let expected =
            "\n--- Dry Run: Files that would be processed ---\n- `file.txt`\n--- End Dry Run ---\n";
        assert_eq!(output, expected);
        Ok(())
    }
}
