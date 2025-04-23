// tests/dry_run.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(sub.join("b.rs"), "B")?;

    dircat_cmd()
        .arg("-D") // Dry run
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            // Check header
            "--- Dry Run: Files that would be processed ---",
        ))
        .stdout(predicate::str::contains("- a.txt")) // Check files (order matters due to sorting)
        .stdout(predicate::str::contains("- sub/b.rs"))
        .stdout(predicate::str::contains("--- End Dry Run ---")) // Check footer
        .stdout(predicate::str::contains("```").not()) // No content blocks
        .stdout(predicate::str::contains("## File:").not()); // No file headers

    temp.close()?;
    Ok(())
}
