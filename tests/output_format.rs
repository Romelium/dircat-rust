// tests/output_format.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_output_single_file_no_extra_newlines() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "Content A")?;

    let expected_output = "## File: a.txt\n```txt\nContent A\n```\n";

    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}

#[test]
fn test_output_multiple_files_separators() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("b.txt"), "Content B")?;
    fs::write(temp.path().join("a.txt"), "Content A")?;

    // The new `discover` function sorts alphabetically before returning the list.
    let expected_output =
        "## File: a.txt\n```txt\nContent A\n```\n\n## File: b.txt\n```txt\nContent B\n```\n";

    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output)); // Use contains due to potential sorting differences

    temp.close()?;
    Ok(())
}

#[test]
fn test_output_single_file_with_summary_separator() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "Content A")?;

    let expected_output =
        "## File: a.txt\n```txt\nContent A\n```\n\n---\nProcessed Files: (1)\n- a.txt\n";

    dircat_cmd()
        .arg("-s")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}

#[test]
fn test_output_multiple_files_with_summary_separators() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("b.txt"), "Content B")?;
    fs::write(temp.path().join("a.txt"), "Content A")?;

    let expected_output = "## File: a.txt\n```txt\nContent A\n```\n\n## File: b.txt\n```txt\nContent B\n```\n\n---\nProcessed Files: (2)\n- a.txt\n- b.txt\n";

    dircat_cmd()
        .arg("-s")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}

#[test]
fn test_output_no_files_is_empty() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    // No files created

    dircat_cmd()
        .current_dir(temp.path())
        .env("RUST_LOG", "off") // Disable logging to ensure stderr only contains the error message
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.\n",
        ));

    temp.close()?;
    Ok(())
}
