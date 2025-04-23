// tests/format_lines.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
// Removed unused: use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_line_numbers() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("lines.txt"), "Line 1\nLine 2\n\nLine 4")?;

    dircat_cmd()
        .arg("-L") // Line numbers
        .current_dir(temp.path())
        .assert()
        .success()
        // Need to be careful with padding - assert presence is safer
        .stdout(predicate::str::contains(" 1 | Line 1"))
        .stdout(predicate::str::contains(" 2 | Line 2"))
        .stdout(predicate::str::contains(" 3 | \n")) // Check empty line 3 explicitly
        .stdout(predicate::str::contains(" 4 | Line 4"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_remove_empty_lines() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("lines.txt"), "Line 1\n  \nLine 3\n\n")?; // Includes whitespace line

    dircat_cmd()
        .arg("-l") // Remove empty lines
        .current_dir(temp.path())
        .assert()
        .success()
        // Exact check to ensure only non-empty lines remain
        .stdout(predicate::str::contains("Line 1\nLine 3\n```"));

    temp.close()?;
    Ok(())
}
