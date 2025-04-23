// tests/format_backticks.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_backticks_in_header() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    dircat_cmd()
        .arg("-b") // Enable backticks
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: `file.txt`")) // Header has backticks
        .stdout(predicate::str::contains("Content"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_backticks_in_summary() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file1.txt"), "Content1")?;
    fs::write(temp.path().join("file2 space.md"), "Content2")?;

    dircat_cmd()
        .arg("-b") // Enable backticks
        .arg("-s") // Enable summary
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: `file1.txt`")) // Header check
        .stdout(predicate::str::contains("## File: `file2 space.md`")) // Header check
        .stdout(predicate::str::contains("\n---\nProcessed Files: (2)\n"))
        .stdout(predicate::str::contains("- `file1.txt`\n")) // Summary item has backticks
        .stdout(predicate::str::contains("- `file2 space.md`\n")); // Summary item has backticks

    temp.close()?;
    Ok(())
}

#[test]
fn test_backticks_in_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    dircat_cmd()
        .arg("-b") // Enable backticks
        .arg("-D") // Dry run
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "--- Dry Run: Files that would be processed ---",
        ))
        .stdout(predicate::str::contains("- `file.txt`")) // Dry run item has backticks
        .stdout(predicate::str::contains("--- End Dry Run ---"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_backticks_with_filename_spaces() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_with_space = "my file with spaces.txt";
    fs::write(temp.path().join(file_with_space), "Space Content")?;

    dircat_cmd()
        .arg("-b") // Enable backticks
        .arg("-s") // Enable summary
        .current_dir(temp.path())
        .assert()
        .success()
        // Check header format
        .stdout(predicate::str::contains(format!(
            "## File: `{}`",
            file_with_space
        )))
        // Check summary format
        .stdout(predicate::str::contains(format!(
            "- `{}`\n",
            file_with_space
        )));

    temp.close()?;
    Ok(())
}
