// tests/filter_regex.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_path_regex_filter() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_dir = temp.path().join("src");
    let test_dir = temp.path().join("tests");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&test_dir)?;

    let main_content = "Main";
    let lib_content = "Lib";
    let test_content = "Test";
    let readme_content = "Readme";
    fs::write(src_dir.join("main.rs"), main_content)?;
    fs::write(src_dir.join("lib.rs"), lib_content)?;
    fs::write(test_dir.join("integration.rs"), test_content)?;
    fs::write(temp.path().join("README.md"), readme_content)?;

    dircat_cmd()
        .arg("-r")
        .arg(r"^src/.*\.rs$") // Match .rs files RELATIVE to input dir starting with src/
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/main.rs")) // Keep header
        .stdout(predicate::str::contains(main_content)) // Keep content
        .stdout(predicate::str::contains("## File: src/lib.rs")) // Keep header
        .stdout(predicate::str::contains(lib_content)) // Keep content
        .stdout(predicate::str::contains("## File: tests/integration.rs").not()) // Ignore header
        .stdout(predicate::str::contains(test_content).not()) // Ignore content
        .stdout(predicate::str::contains("## File: README.md").not()) // Ignore header
        .stdout(predicate::str::contains(readme_content).not()); // Ignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_filename_regex_filter() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_dir = temp.path().join("src");
    fs::create_dir_all(&src_dir)?;

    let main_content = "Main";
    let lib_content = "Lib";
    let config_content = "Config";
    let cargo_content = "Cargo";
    fs::write(src_dir.join("main.rs"), main_content)?;
    fs::write(src_dir.join("lib.rs"), lib_content)?;
    fs::write(temp.path().join("config.toml"), config_content)?;
    fs::write(temp.path().join("Cargo.toml"), cargo_content)?;

    dircat_cmd()
        .arg("-d")
        .arg(r"\.toml$") // Match only filenames ending in .toml
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: config.toml")) // Keep header
        .stdout(predicate::str::contains(config_content)) // Keep content
        .stdout(predicate::str::contains("## File: Cargo.toml")) // Keep header
        .stdout(predicate::str::contains(cargo_content)) // Keep content
        .stdout(predicate::str::contains("## File: src/main.rs").not()) // Ignore header
        .stdout(predicate::str::contains(main_content).not()) // Ignore content
        .stdout(predicate::str::contains("## File: src/lib.rs").not()) // Ignore header
        .stdout(predicate::str::contains(lib_content).not()); // Ignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_path_and_filename_regex_filters() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_dir = temp.path().join("src");
    let test_dir = temp.path().join("tests");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&test_dir)?;

    let main_content = "Main";
    let src_config = "SrcConfig";
    let test_config = "TestConfig";
    let cargo_content = "Cargo";
    fs::write(src_dir.join("main.rs"), main_content)?;
    fs::write(src_dir.join("config.toml"), src_config)?;
    fs::write(test_dir.join("test_config.toml"), test_config)?;
    fs::write(temp.path().join("Cargo.toml"), cargo_content)?;

    dircat_cmd()
        .arg("-r")
        .arg(r"^src/") // Relative path must start with src/
        .arg("-d")
        .arg(r"\.toml$") // Filename must end in .toml
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/config.toml")) // Only match header
        .stdout(predicate::str::contains(src_config)) // Only match content
        .stdout(predicate::str::contains("## File: src/main.rs").not()) // Ignore header
        .stdout(predicate::str::contains("## File: tests/test_config.toml").not()) // Ignore header
        .stdout(predicate::str::contains("## File: Cargo.toml").not()); // Ignore header

    temp.close()?;
    Ok(())
}

#[test]
fn test_path_regex_windows_separators() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_dir = temp.path().join("src");
    let data_dir = temp.path().join("data");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&data_dir)?;

    let main_content = "Main";
    let data_content = "Data";
    fs::write(src_dir.join("main.rs"), main_content)?;
    fs::write(data_dir.join("config.txt"), data_content)?;

    // Use a regex with forward slashes, even if running on Windows.
    // The internal logic normalizes relative paths to use forward slashes before matching.
    dircat_cmd()
        .arg("-r")
        .arg("^src/") // Match relative paths starting with src/
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/main.rs")) // Keep header
        .stdout(predicate::str::contains(main_content)) // Keep content
        .stdout(predicate::str::contains("## File: data/config.txt").not()) // Ignore header
        .stdout(predicate::str::contains(data_content).not()); // Ignore content

    temp.close()?;
    Ok(())
}
