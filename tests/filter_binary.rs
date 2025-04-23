// tests/filter_binary.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_skip_binary_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let text_content = "This is text.";
    let binary_content = b"This has a null \0 byte.";
    fs::write(temp.path().join("text.txt"), text_content)?;
    fs::File::create(temp.path().join("binary.bin"))?.write_all(binary_content)?;

    dircat_cmd()
        // No --include-binary flag
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: text.txt")) // Keep text
        .stdout(predicate::str::contains(text_content))
        .stdout(predicate::str::contains("## File: binary.bin").not()) // Skip binary
        .stdout(predicate::str::contains("This has a null").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_include_binary_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let text_content = "This is text.";
    let binary_content = b"This has a null \0 byte.";
    let binary_content_str = String::from_utf8_lossy(binary_content); // How it will appear in output
    fs::write(temp.path().join("text.txt"), text_content)?;
    fs::File::create(temp.path().join("binary.bin"))?.write_all(binary_content)?;

    dircat_cmd()
        .arg("--include-binary") // Include binary files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: text.txt")) // Keep text
        .stdout(predicate::str::contains(text_content))
        .stdout(predicate::str::contains("## File: binary.bin")) // Keep binary header
        .stdout(predicate::str::contains(binary_content_str.as_ref())); // Keep binary content (lossy)

    temp.close()?;
    Ok(())
}

#[test]
fn test_skip_image_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let png_magic_bytes = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    fs::write(temp.path().join("text.txt"), "Text")?;
    fs::File::create(temp.path().join("image.png"))?.write_all(png_magic_bytes)?;

    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: text.txt")) // Keep text
        .stdout(predicate::str::contains("## File: image.png").not()); // Skip image

    temp.close()?;
    Ok(())
}

#[test]
fn test_include_image_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let png_magic_bytes = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    // let png_content_str = String::from_utf8_lossy(png_magic_bytes); // No longer needed for exact match
    fs::write(temp.path().join("text.txt"), "Text")?;
    fs::File::create(temp.path().join("image.png"))?.write_all(png_magic_bytes)?;

    dircat_cmd()
        .arg("--include-binary")
        .current_dir(temp.path())
        .assert()
        .success() // Should now succeed
        .stdout(predicate::str::contains("## File: text.txt")) // Keep text
        .stdout(predicate::str::contains("## File: image.png")) // Keep image header
        // Check that the code block contains the printable part of the magic bytes
        .stdout(predicate::str::contains("```png\n").and(predicate::str::contains("PNG")));

    temp.close()?;
    Ok(())
}
