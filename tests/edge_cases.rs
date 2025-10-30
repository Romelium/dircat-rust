// tests/edge_cases.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_input_path_is_file_with_filters() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_path = temp.path().join("input.rs");
    let file_content = "fn main() {}";
    fs::write(&file_path, file_content)?;

    // Test with a matching filter
    dircat_cmd()
        .arg(file_path.to_str().unwrap())
        .arg("-e")
        .arg("rs") // Include .rs
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: input.rs"))
        .stdout(predicate::str::contains(file_content));

    // Test with a non-matching filter
    dircat_cmd()
        .arg(file_path.to_str().unwrap())
        .arg("-e")
        .arg("txt") // Include .txt (won't match)
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File:").not())
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_dir_with_only_ignored_files() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    fs::write(temp.path().join("a.log"), "Log A")?;
    fs::write(temp.path().join("b.log"), "Log B")?;

    dircat_cmd()
        // Default behavior respects .gitignore
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File:").not())
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_dir_with_only_binary_files() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let bin1 = b"Bin\0One";
    let bin2 = &[0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
    fs::File::create(temp.path().join("file1.bin"))?.write_all(bin1)?;
    fs::File::create(temp.path().join("image.png"))?.write_all(bin2)?;

    // Default behavior skips binaries
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File:").not())
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        ));

    // With -B, include binaries
    dircat_cmd()
        .arg("-B")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: file1.bin"))
        .stdout(predicate::str::contains(
            String::from_utf8_lossy(bin1).as_ref(),
        ))
        .stdout(predicate::str::contains("## File: image.png"))
        .stdout(predicate::str::contains(
            String::from_utf8_lossy(bin2).as_ref(),
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_dir_with_only_lockfiles() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("Cargo.lock"), "Cargo")?;
    fs::write(temp.path().join("yarn.lock"), "Yarn")?;

    // Default behavior includes lockfiles
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: Cargo.lock"))
        .stdout(predicate::str::contains("## File: yarn.lock"));

    // With -K, skip lockfiles
    dircat_cmd()
        .arg("-K")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File:").not())
        .stderr(predicate::str::contains(
            "dircat: No files found matching the specified criteria.",
        ));

    temp.close()?;
    Ok(())
}

#[test]
fn test_multiple_include_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.rs"), "Rust")?;
    fs::write(temp.path().join("b.toml"), "Toml")?;
    fs::write(temp.path().join("c.txt"), "Text")?;

    dircat_cmd()
        .arg("-e")
        .arg("rs")
        .arg("-e")
        .arg("toml") // Multiple -e flags
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: a.rs"))
        .stdout(predicate::str::contains("## File: b.toml"))
        .stdout(predicate::str::contains("## File: c.txt").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_multiple_exclude_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.log"), "Log")?;
    fs::write(temp.path().join("b.tmp"), "Temp")?;
    fs::write(temp.path().join("c.txt"), "Text")?;

    dircat_cmd()
        .arg("-x")
        .arg("log")
        .arg("-x")
        .arg("tmp") // Multiple -x flags
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: c.txt"))
        .stdout(predicate::str::contains("## File: a.log").not())
        .stdout(predicate::str::contains("## File: b.tmp").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_multiple_path_regex() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    let tests = temp.path().join("tests");
    let data = temp.path().join("data");
    fs::create_dir_all(&src)?;
    fs::create_dir_all(&tests)?;
    fs::create_dir_all(&data)?;
    fs::write(src.join("main.rs"), "Src")?;
    fs::write(tests.join("unit.rs"), "Tests")?;
    fs::write(data.join("config.txt"), "Data")?;

    dircat_cmd()
        .arg("-r")
        .arg("^src/") // Match src dir
        .arg("-r")
        .arg("^tests/") // Match tests dir
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/main.rs"))
        .stdout(predicate::str::contains("## File: tests/unit.rs"))
        .stdout(predicate::str::contains("## File: data/config.txt").not());

    temp.close()?;
    Ok(())
}

/// Tests that symbolic links are ignored by default, which is the desired behavior
/// to prevent recursion issues and processing files outside the target directory.
#[test]
#[cfg(unix)] // Symlinks are a Unix-specific feature in Rust's standard library
fn test_symlinks_are_ignored_by_default() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let temp = tempdir()?;
    let target_content = "This is the real file.";
    let target_path = temp.path().join("target.txt");
    fs::write(&target_path, target_content)?;

    let link_path = temp.path().join("link.txt");
    symlink(&target_path, &link_path)?;

    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: target.txt")) // The real file should be included
        .stdout(predicate::str::contains(target_content))
        .stdout(predicate::str::contains("## File: link.txt").not()); // The symlink itself should be ignored

    temp.close()?;
    Ok(())
}
