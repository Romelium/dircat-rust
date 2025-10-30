// tests/filter_combinations_adv.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_filter_ext_include_exclude_precedence() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "Text A")?;
    fs::write(temp.path().join("b.log"), "Log B")?;
    fs::write(temp.path().join("c.txt"), "Text C")?;

    dircat_cmd()
        .arg("-e")
        .arg("txt") // Include .txt
        .arg("-x")
        .arg("txt") // Also exclude .txt (exclude should win)
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File:").not()) // No files should be included
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        )); // Expect the "no files found" message

    temp.close()?;
    Ok(())
}

#[test]
fn test_filter_ext_include_and_path_regex() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let docs = temp.path().join("docs");
    fs::create_dir_all(&src)?;
    fs::create_dir_all(&docs)?;
    fs::write(src.join("main.rs"), "Rust Main")?;
    fs::write(src.join("lib.txt"), "Text Lib")?; // Wrong extension
    fs::write(docs.join("guide.rs"), "Rust Guide")?; // Wrong path

    dircat_cmd()
        .arg("-e")
        .arg("rs") // Include .rs
        .arg("-r")
        .arg("^src/") // Path must start with src/
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/main.rs")) // Keep
        .stdout(predicate::str::contains("Rust Main"))
        .stdout(predicate::str::contains("## File: src/lib.txt").not()) // Excluded by ext
        .stdout(predicate::str::contains("## File: docs/guide.rs").not()); // Excluded by path

    temp.close()?;
    Ok(())
}

#[test]
fn test_filter_ext_exclude_and_filename_regex() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("test_a.log"), "Test Log A")?;
    fs::write(temp.path().join("test_b.txt"), "Test Text B")?;
    fs::write(temp.path().join("prod_c.log"), "Prod Log C")?;

    dircat_cmd()
        .arg("-x")
        .arg("log") // Exclude .log
        .arg("-d")
        .arg("^test_") // Filename must start with test_
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: test_b.txt")) // Keep
        .stdout(predicate::str::contains("Test Text B"))
        .stdout(predicate::str::contains("## File: test_a.log").not()) // Excluded by ext
        .stdout(predicate::str::contains("## File: prod_c.log").not()); // Excluded by filename regex

    temp.close()?;
    Ok(())
}

#[test]
fn test_filter_max_size_and_ext_include() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("small.rs"), "fn s(){}")?; // Small, .rs
    fs::write(temp.path().join("large.rs"), "fn l(){}".repeat(50))?; // Large, .rs
    fs::write(temp.path().join("small.txt"), "text")?; // Small, .txt

    dircat_cmd()
        .arg("-m")
        .arg("50") // Max 50 bytes
        .arg("-e")
        .arg("rs") // Include .rs
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: small.rs")) // Keep
        .stdout(predicate::str::contains("fn s(){}"))
        .stdout(predicate::str::contains("## File: large.rs").not()) // Excluded by size
        .stdout(predicate::str::contains("## File: small.txt").not()); // Excluded by ext

    temp.close()?;
    Ok(())
}

#[test]
fn test_filter_no_lockfiles_and_ext_include() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("Cargo.lock"), "Cargo Lock")?;
    fs::write(temp.path().join("yarn.lock"), "Yarn Lock")?;
    fs::write(temp.path().join("other.lock"), "Other Lock")?; // Custom lock file

    dircat_cmd()
        .arg("-K") // Skip common lockfiles
        .arg("-e")
        .arg("lock") // Try to include *.lock files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: other.lock")) // Keep custom lock
        .stdout(predicate::str::contains("Other Lock"))
        .stdout(predicate::str::contains("## File: Cargo.lock").not()) // Excluded by -K
        .stdout(predicate::str::contains("## File: yarn.lock").not()); // Excluded by -K

    temp.close()?;
    Ok(())
}

#[test]
fn test_filter_include_binary_and_ext_include() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let binary_content = b"Binary\0Data";
    fs::File::create(temp.path().join("data.bin"))?.write_all(binary_content)?;
    fs::write(temp.path().join("config.bin"), "Text Config")?; // Text file with .bin ext
    fs::write(temp.path().join("image.png"), "Fake PNG")?;

    dircat_cmd()
        .arg("-B") // Include binary
        .arg("-e")
        .arg("bin") // Include only .bin files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: data.bin")) // Keep binary .bin
        .stdout(predicate::str::contains(
            String::from_utf8_lossy(binary_content).as_ref(),
        ))
        .stdout(predicate::str::contains("## File: config.bin")) // Keep text .bin
        .stdout(predicate::str::contains("Text Config"))
        .stdout(predicate::str::contains("## File: image.png").not()); // Excluded by ext

    temp.close()?;
    Ok(())
}

/// Tests that an exclusion filter (like --exclude-ext) takes precedence over
/// a processing order instruction (like --last). A file should be fully excluded
/// even if it's also marked to be processed last.
#[test]
fn test_filter_last_is_overridden_by_exclude_ext() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "Text A")?;
    fs::write(temp.path().join("README.md"), "Important Readme")?;

    dircat_cmd()
        .arg("-z")
        .arg("README.md") // Try to process README last
        .arg("-x")
        .arg("md") // But also exclude all markdown files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: a.txt")) // Keep a.txt
        .stdout(predicate::str::contains("Text A"))
        .stdout(predicate::str::contains("## File: README.md").not()); // README should be excluded

    temp.close()?;
    Ok(())
}
