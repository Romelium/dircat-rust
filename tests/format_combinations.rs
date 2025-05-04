// tests/format_combinations.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_format_lines_and_filename_only() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(sub.join("file.txt"), "Line 1\nLine 2")?;

    dircat_cmd()
        .arg("-L") // Line numbers
        .arg("-f") // Filename only
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: file.txt")) // Filename only header
        .stdout(predicate::str::contains(" 1 | Line 1")) // Line numbers present
        .stdout(predicate::str::contains(" 2 | Line 2"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_format_lines_and_backticks() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Line 1\nLine 2")?;

    dircat_cmd()
        .arg("-L") // Line numbers
        .arg("-b") // Backticks
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: `file.txt`")) // Backticked header
        .stdout(predicate::str::contains(" 1 | Line 1")) // Line numbers present
        .stdout(predicate::str::contains(" 2 | Line 2"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_format_filename_only_and_backticks() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(sub.join("file.txt"), "Content")?;

    dircat_cmd()
        .arg("-f") // Filename only
        .arg("-b") // Backticks
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: `file.txt`")) // Filename only and backticked header
        .stdout(predicate::str::contains("Content"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_format_lines_filename_only_and_backticks() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(sub.join("file.txt"), "Line 1\nLine 2")?;

    dircat_cmd()
        .arg("-L") // Line numbers
        .arg("-f") // Filename only
        .arg("-b") // Backticks
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: `file.txt`")) // Filename only and backticked header
        .stdout(predicate::str::contains(" 1 | Line 1")) // Line numbers present
        .stdout(predicate::str::contains(" 2 | Line 2"));

    temp.close()?;
    Ok(())
}
