// tests/summary_counts.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write; // Needed for binary write
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

#[test]
fn test_summary_with_counts_and_binary() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let text_content = "One two"; // 1 line, 7 chars, 2 words
    let binary_content = b"Binary\0Data"; // 11 bytes
    fs::write(temp.path().join("text.txt"), text_content)?;
    fs::File::create(temp.path().join("binary.bin"))?.write_all(binary_content)?;

    dircat_cmd()
        .arg("--counts")
        .arg("-s")
        .arg("--include-binary") // Include the binary file
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: text.txt"))
        .stdout(predicate::str::contains(text_content))
        .stdout(predicate::str::contains("## File: binary.bin"))
        // *** Fix: Convert Cow to String before passing to contains ***
        .stdout(predicate::str::contains(
            String::from_utf8_lossy(binary_content).to_string(), // Use .to_string() here
        ))
        .stdout(predicate::str::contains("\n---\nProcessed Files: (2)\n"))
        // Check summary items (sorted alphabetically) with correct formats
        .stdout(predicate::str::contains("- binary.bin (Binary C:11)\n"))
        .stdout(predicate::str::contains("- text.txt (L:1 C:7 W:2)\n"));

    temp.close()?;
    Ok(())
}
