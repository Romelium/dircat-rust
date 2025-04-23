// tests/summary_counts.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_summary() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    let content_z = "Z";
    let content_a = "A";
    fs::write(temp.path().join("z.txt"), content_z)?;
    fs::write(sub.join("a.rs"), content_a)?;

    dircat_cmd()
        .arg("-s") // Summary
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/a.rs")) // Check headers present
        .stdout(predicate::str::contains("## File: z.txt"))
        .stdout(predicate::str::contains(content_a)) // Check content present
        .stdout(predicate::str::contains(content_z))
        .stdout(predicate::str::contains("\n---\nProcessed Files: (2)\n")) // Check summary header
        .stdout(predicate::str::contains("- sub/a.rs\n")) // Check summary items (sorted alphabetically)
        .stdout(predicate::str::contains("- z.txt\n"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_summary_with_counts() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let content = "One two\nThree"; // 2 lines, 13 chars, 3 words
    fs::write(temp.path().join("file.txt"), content)?;

    dircat_cmd()
        .arg("--counts") // Counts
        .arg("-s") // Explicitly add -s to satisfy clap's 'requires' rule
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: file.txt")) // Check header
        .stdout(predicate::str::contains(content)) // Check content
        .stdout(predicate::str::contains("\n---\nProcessed Files: (1)\n")) // Check summary header
        .stdout(predicate::str::contains("- file.txt (L:2 C:13 W:3)\n")); // Check summary item with counts

    temp.close()?;
    Ok(())
}
