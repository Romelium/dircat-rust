mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dry_run_respects_last_order() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    // Create two files
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(temp.path().join("z.txt"), "Z")?;

    // We request 'a.txt' to be processed LAST.
    // Expected Order: 
    // 1. z.txt (normal file)
    // 2. a.txt (last file)
    
    dircat_cmd()
        .arg("-D") // Dry run
        .arg("--last")
        .arg("a.txt")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            let pos_a = output.find("- a.txt").unwrap_or(usize::MAX);
            let pos_z = output.find("- z.txt").unwrap_or(usize::MAX);
            
            if pos_a == usize::MAX || pos_z == usize::MAX {
                return false;
            }

            pos_z < pos_a
        }));

    temp.close()?;
    Ok(())
}