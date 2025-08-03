// tests/output_file.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
// Removed unused: use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;
// *** Fix: Removed unused imports ***
// use dircat::core_types::{FileInfo, FileCounts}; // Import FileInfo and FileCounts
// Removed unused import: use std::path::PathBuf;

#[test]
fn test_output_to_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_path = temp.path().join("src");
    fs::create_dir(&src_path)?;
    let content_a = "Content A";
    fs::write(src_path.join("a.txt"), content_a)?;
    let output_path = temp.path().join("output.md");

    dircat_cmd()
        .arg(src_path.to_str().unwrap()) // Input is the 'src' directory
        .arg("-o")
        .arg(output_path.to_str().unwrap())
        .current_dir(temp.path()) // Run from parent directory of 'src'
        .assert()
        .success()
        .stdout(""); // No stdout expected

    // Read the output file and assert its contents
    let output_content = fs::read_to_string(&output_path)?;
    assert!(output_content.contains("## File: a.txt")); // Check file header (relative to input dir 'src')
    assert!(output_content.contains("```txt")); // Check lang hint
    assert!(output_content.contains(content_a)); // Check content
    assert!(output_content.contains("```")); // Check closing fence

    temp.close()?;
    Ok(())
}
