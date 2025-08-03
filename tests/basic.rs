mod common; // Declare the common module

use assert_cmd::prelude::*;
use common::dircat_cmd; // Import the helper
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_no_args_uses_current_dir() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_path = temp.path().join("test.txt");
    let file_content = "Hello";
    fs::write(&file_path, file_content)?;

    dircat_cmd()
        .current_dir(temp.path()) // Run in the temp dir
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: test.txt")) // Check file header
        .stdout(predicate::str::contains("```txt")) // Check language hint
        .stdout(predicate::str::contains(file_content)) // Check content presence
        .stdout(predicate::str::contains("```")); // Check closing fence

    temp.close()?;
    Ok(())
}

#[test]
fn test_specific_file_input() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_path = temp.path().join("input.rs");
    let file_content = "fn main() {}";
    fs::write(&file_path, file_content)?;

    dircat_cmd()
        .arg(file_path.to_str().unwrap()) // Pass file path as arg
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: input.rs")) // Check correct relative path (filename)
        .stdout(predicate::str::contains("```rs")) // Check language hint
        .stdout(predicate::str::contains(file_content)) // Check content
        .stdout(predicate::str::contains("```"));

    temp.close()?;
    Ok(())
}
