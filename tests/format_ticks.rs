// tests/format_ticks.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_default_ticks() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    let expected_output = "## File: file.txt\n```txt\nContent\n```\n";

    dircat_cmd()
        // No --ticks flag, should default to 3
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}

#[test]
fn test_custom_ticks() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    let expected_output = "## File: file.txt\n`````txt\nContent\n`````\n";

    dircat_cmd()
        .arg("--ticks")
        .arg("5")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}

#[test]
fn test_invalid_ticks_error() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    dircat_cmd()
        .arg("--ticks")
        .arg("2") // Invalid, must be >= 3
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "The number of ticks must be 3 or greater.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_custom_ticks_alias() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("file.txt"), "Content")?;

    let expected_output = "## File: file.txt\n````txt\nContent\n````\n";

    dircat_cmd()
        .arg("-T") // Use the short alias
        .arg("4")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::eq(expected_output));

    temp.close()?;
    Ok(())
}
