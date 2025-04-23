// tests/errors.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
// Removed unused: use std::fs;
use std::fs;
use std::io::Write;
use tempfile::tempdir; // Added for writing non-UTF8 bytes

#[test]
fn test_error_invalid_input_path() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?; // Need a valid directory to run from

    dircat_cmd()
        .arg("non_existent_path_hopefully")
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to resolve input path"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_error_output_and_paste_conflict() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;

    dircat_cmd()
        .arg("-o")
        .arg("output.txt")
        .arg("-p") // Conflict
        .current_dir(temp.path())
        .assert()
        .failure()
        // This error comes from our custom validation logic
        .stderr(predicate::str::contains(
            "Cannot use --output <FILE> (-o) and --paste (-p) simultaneously",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_error_invalid_max_size_format() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;

    dircat_cmd()
        .arg("-m")
        .arg("invalid_size") // Invalid format
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid size format"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_error_invalid_regex_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;

    dircat_cmd()
        .arg("-r")
        .arg("[invalid") // Invalid regex
        .current_dir(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid path regex"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_error_no_files_found() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    // Create a directory but no files matching default criteria
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(sub.join(".hidden"), "content")?; // Hidden file, ignored by default

    dircat_cmd()
        .current_dir(temp.path()) // Run in the temp dir with only hidden files/dirs
        .assert()
        .success() // Should still succeed, but print message to stderr
        .stdout("") // No files processed, so stdout should be empty
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
#[cfg(not(feature = "clipboard"))] // Only run this test if clipboard feature is NOT enabled
fn test_error_paste_feature_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;

    dircat_cmd()
        .arg("-p") // Use paste flag
        .current_dir(temp.path())
        .assert()
        .failure()
        // Check for the specific error message from finalize_output -> copy_to_clipboard
        .stderr(predicate::str::contains(
            "Clipboard feature is not enabled, cannot use --paste.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_include_invalid_utf8_lossy() -> Result<(), Box<dyn std::error::Error>> {
    // Renamed test: This now tests successful inclusion with lossy conversion
    let temp = tempdir()?;
    let file_path = temp.path().join("invalid_utf8.bin");
    let invalid_bytes = &[0x48, 0x65, 0x6c, 0x6c, 0x80, 0x6f]; // "Hell\x80o"
    fs::File::create(&file_path)?.write_all(invalid_bytes)?;

    // Expected lossy conversion (0x80 becomes U+FFFD REPLACEMENT CHARACTER)
    let expected_lossy_content = "Hell\u{FFFD}o";

    dircat_cmd()
        .arg(file_path.to_str().unwrap())
        .arg("--include-binary") // Force inclusion
        .current_dir(temp.path())
        .assert()
        .success() // Expect success because lossy conversion handles the error
        .stdout(predicate::str::contains("## File: invalid_utf8.bin")) // Check header
        .stdout(predicate::str::contains(expected_lossy_content)); // Check for lossy content
                                                                   // Removed: .stderr("") - Do not assert empty stderr as logs might be present

    temp.close()?;
    Ok(())
}

// Note: Testing clipboard errors (-p without feature or clipboard unavailable)
// is harder in CI and depends on the feature flag.
