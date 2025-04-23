// tests/filter_lockfiles.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_skip_lockfiles_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("Cargo.lock"), "Cargo Lock Content")?;
    fs::write(temp.path().join("package-lock.json"), "NPM Lock Content")?;
    fs::write(temp.path().join("src.rs"), "Source Content")?;

    dircat_cmd()
        // No --no-lockfiles flag, should keep lockfiles
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: Cargo.lock")) // Keep lockfile
        .stdout(predicate::str::contains("Cargo Lock Content"))
        .stdout(predicate::str::contains("## File: package-lock.json")) // Keep lockfile
        .stdout(predicate::str::contains("NPM Lock Content"))
        .stdout(predicate::str::contains("## File: src.rs")) // Keep source
        .stdout(predicate::str::contains("Source Content"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_no_lockfiles_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("Cargo.lock"), "Cargo Lock Content")?;
    fs::write(temp.path().join("package-lock.json"), "NPM Lock Content")?;
    fs::write(temp.path().join("src.rs"), "Source Content")?;

    dircat_cmd()
        .arg("--no-lockfiles") // Skip lockfiles
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: Cargo.lock").not()) // Skip lockfile
        .stdout(predicate::str::contains("Cargo Lock Content").not())
        .stdout(predicate::str::contains("## File: package-lock.json").not()) // Skip lockfile
        .stdout(predicate::str::contains("NPM Lock Content").not())
        .stdout(predicate::str::contains("## File: src.rs")) // Keep source
        .stdout(predicate::str::contains("Source Content"));

    temp.close()?;
    Ok(())
}
