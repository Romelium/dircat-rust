// tests/format_header.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_filename_only_header() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub_path = temp.path().join("sub");
    fs::create_dir(&sub_path)?;
    let content = "Content";
    fs::write(sub_path.join("a.txt"), content)?;

    dircat_cmd()
        .arg("-f") // Filename only
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: a.txt")) // Just filename
        .stdout(predicate::str::contains(content)) // Check content
        .stdout(predicate::str::contains("## File: sub/a.txt").not()); // Not full path

    temp.close()?;
    Ok(())
}
