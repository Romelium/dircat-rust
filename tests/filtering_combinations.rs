// tests/filtering_combinations.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_combine_size_ext_regex() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src_dir = temp.path().join("src");
    let data_dir = temp.path().join("data");
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&data_dir)?;

    // Files to potentially include
    let small_code_content = "fn s(){}";
    let large_code_content = "fn l(){}".repeat(1000);
    let small_config_content = "[table]";
    let small_data_content = "// data";
    let other_content = "text";

    fs::write(src_dir.join("small_code.rs"), small_code_content)?; // Small, .rs, in src/
    fs::write(src_dir.join("large_code.rs"), &large_code_content)?; // Large, .rs, in src/
    fs::write(src_dir.join("small_config.toml"), small_config_content)?; // Small, .toml, in src/
    fs::write(data_dir.join("small_data.rs"), small_data_content)?; // Small, .rs, not in src/
    fs::write(data_dir.join("other.txt"), other_content)?; // Small, .txt, not in src/

    dircat_cmd()
        .arg("-m")
        .arg("100") // Max size 100 bytes
        .arg("-e")
        .arg("rs") // Only include .rs extension
        .arg("-r")
        .arg("^src/") // Path must start with src/ (relative to temp dir)
        .current_dir(temp.path())
        .assert()
        .success()
        // Only small_code.rs should match all criteria
        .stdout(predicate::str::contains("## File: src/small_code.rs")) // Check header
        .stdout(predicate::str::contains(small_code_content)) // Check content
        // Check that others are excluded
        .stdout(predicate::str::contains("## File: src/large_code.rs").not()) // Excluded by size
        .stdout(predicate::str::contains(&large_code_content).not())
        .stdout(predicate::str::contains("## File: src/small_config.toml").not()) // Excluded by extension
        .stdout(predicate::str::contains(small_config_content).not())
        .stdout(predicate::str::contains("## File: data/small_data.rs").not()) // Excluded by path regex
        .stdout(predicate::str::contains(small_data_content).not())
        .stdout(predicate::str::contains("## File: data/other.txt").not()) // Excluded by ext & regex
        .stdout(predicate::str::contains(other_content).not());

    temp.close()?;
    Ok(())
}
