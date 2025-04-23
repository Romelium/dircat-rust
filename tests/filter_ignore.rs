mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_ignore_glob_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let logs_dir = temp.path().join("logs");
    let target_dir = temp.path().join("target");
    let src_dir = temp.path().join("src");
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&target_dir)?;
    fs::create_dir_all(&src_dir)?;

    let main_content = "Main";
    let lib_content = "Lib";
    let log_content = "Log";
    let debug_content = "Debug Build";
    let temp_content = "Temp";
    fs::write(temp.path().join("main.rs"), main_content)?;
    fs::write(src_dir.join("lib.rs"), lib_content)?;
    fs::write(logs_dir.join("app.log"), log_content)?;
    fs::write(target_dir.join("debug"), debug_content)?;
    fs::write(temp.path().join("temp_file.tmp"), temp_content)?;

    dircat_cmd()
        .arg("-i")
        .arg("*.log") // Ignore log files anywhere
        .arg("-i")
        .arg("target/*") // Ignore anything directly under target/
        .arg("-i")
        .arg("*.tmp") // Ignore temp files
        .arg("-t") // Disable default .gitignore handling for this test
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: main.rs")) // Keep header
        .stdout(predicate::str::contains(main_content)) // Keep content
        .stdout(predicate::str::contains("## File: src/lib.rs")) // Keep header
        .stdout(predicate::str::contains(lib_content)) // Keep content
        .stdout(predicate::str::contains("## File: logs/app.log").not()) // Ignore header
        .stdout(predicate::str::contains(log_content).not()) // Ignore content
        .stdout(predicate::str::contains("## File: target/debug").not()) // Ignore header
        .stdout(predicate::str::contains(debug_content).not()) // Ignore content
        .stdout(predicate::str::contains("## File: temp_file.tmp").not()) // Ignore header
        .stdout(predicate::str::contains(temp_content).not()); // Ignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_ignore_glob_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let ignore_dir = temp.path().join("ignored_stuff");
    let keep_dir = temp.path().join("kept_stuff");
    fs::create_dir_all(&ignore_dir)?;
    fs::create_dir_all(&keep_dir)?;

    let ignore_content = "Ignore Me";
    let keep_content = "Keep Me";
    let root_content = "Root";
    fs::write(ignore_dir.join("a.txt"), ignore_content)?;
    fs::write(keep_dir.join("b.txt"), keep_content)?;
    fs::write(temp.path().join("root.txt"), root_content)?;

    dircat_cmd()
        .arg("-i")
        .arg("ignored_stuff/**") // Ignore the whole directory and its contents
        .arg("-t") // Disable default .gitignore handling
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: kept_stuff/b.txt")) // Keep header
        .stdout(predicate::str::contains(keep_content)) // Keep content
        .stdout(predicate::str::contains("## File: root.txt")) // Keep header
        .stdout(predicate::str::contains(root_content)) // Keep content
        .stdout(predicate::str::contains("## File: ignored_stuff/a.txt").not()) // Ignore header
        .stdout(predicate::str::contains(ignore_content).not()); // Ignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_ignore_invalid_glob_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let content_a = "A";
    let content_b = "B";
    fs::write(temp.path().join("a.txt"), content_a)?;
    fs::write(temp.path().join("b.log"), content_b)?;

    // Provide one invalid glob and one valid one.
    // Expect a warning on stderr (via logging) and the valid glob to still work.
    dircat_cmd()
        .arg("-i")
        .arg("[invalid") // Invalid glob pattern
        .arg("-i")
        .arg("*.log") // Valid glob pattern
        .arg("-t") // Disable default .gitignore handling
        .env("RUST_LOG", "warn") // Ensure warnings are captured
        .current_dir(temp.path())
        .assert()
        .success() // Command should still succeed
        .stderr(predicate::str::contains(
            "Invalid ignore glob pattern '[invalid'",
        )) // Check for warning
        .stdout(predicate::str::contains("## File: a.txt")) // Keep header
        .stdout(predicate::str::contains(content_a)) // Keep content
        .stdout(predicate::str::contains("## File: b.log").not()) // Ignore header
        .stdout(predicate::str::contains(content_b).not()); // Ignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_ignore_only_invalid_glob_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let content_a = "A";
    let content_b = "B";
    fs::write(temp.path().join("a.txt"), content_a)?;
    fs::write(temp.path().join("b.log"), content_b)?;

    // Provide only invalid globs. Expect warnings but no files ignored by -i.
    dircat_cmd()
        .arg("-i")
        .arg("[invalid1")
        .arg("-i")
        .arg("[invalid2")
        .arg("-t") // Disable default .gitignore handling
        .env("RUST_LOG", "warn") // Ensure warnings are captured
        .current_dir(temp.path())
        .assert()
        .success() // Command should still succeed
        .stderr(predicate::str::contains(
            "Invalid ignore glob pattern '[invalid1'",
        )) // Check warnings
        .stderr(predicate::str::contains(
            "Invalid ignore glob pattern '[invalid2'",
        ))
        .stdout(predicate::str::contains("## File: a.txt")) // Keep header
        .stdout(predicate::str::contains(content_a)) // Keep content
        .stdout(predicate::str::contains("## File: b.log")) // Keep header (no valid ignore patterns)
        .stdout(predicate::str::contains(content_b)); // Keep content

    temp.close()?;
    Ok(())
}
