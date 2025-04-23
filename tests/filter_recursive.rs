// tests/filter_recursive.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_no_recursive() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub_path = temp.path().join("sub");
    fs::create_dir(&sub_path)?;
    fs::write(temp.path().join("a.txt"), "Top Level")?;
    fs::write(sub_path.join("b.txt"), "Sub Level")?;

    dircat_cmd()
        .arg("-n") // No recursive
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: a.txt"))
        .stdout(predicate::str::contains("Top Level"))
        .stdout(predicate::str::contains("## File: sub/b.txt").not()) // Should not contain sub level file
        .stdout(predicate::str::contains("Sub Level").not());

    temp.close()?;
    Ok(())
}
