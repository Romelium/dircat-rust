// tests/filter_gitignore.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd; // Import assert_paths helper if it exists, otherwise keep predicates
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_respects_gitignore_by_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let gitignore_path = temp.path().join(".gitignore");
    let git_dir = temp.path().join(".git"); // Ensure .git is treated as a directory
    let target_dir = temp.path().join("target");
    fs::create_dir_all(&git_dir)?; // Create .git as a directory
    fs::create_dir_all(&target_dir)?;

    // Create .gitignore
    let mut file = fs::File::create(gitignore_path)?;
    let gitignore_content = "target/\n*.log\n";
    write!(file, "{}", gitignore_content)?;
    drop(file); // Ensure file is closed

    // Create files
    let src_content = "Source";
    let obj_content = "Object";
    let log_content = "Log";
    fs::write(temp.path().join("src.rs"), src_content)?;
    fs::write(target_dir.join("debug.o"), obj_content)?;
    fs::write(temp.path().join("app.log"), log_content)?;

    // Use predicates for direct output check
    dircat_cmd()
        // No -t flag, should respect .gitignore
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src.rs")) // Keep source header
        .stdout(predicate::str::contains(src_content)) // Keep source content
        .stdout(predicate::str::contains("## File: target/debug.o").not()) // Ignore target/ header
        .stdout(predicate::str::contains(obj_content).not()) // Ignore target/ content
        .stdout(predicate::str::contains("## File: app.log").not()) // Ignore *.log header
        .stdout(predicate::str::contains(log_content).not()) // Ignore *.log content
        // Also ensure .gitignore and .git are not listed (hidden files ignored by default)
        .stdout(predicate::str::contains("## File: .gitignore").not())
        .stdout(predicate::str::contains("## File: .git").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_no_gitignore_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let gitignore_path = temp.path().join(".gitignore");
    let git_dir = temp.path().join(".git"); // Ensure .git is treated as a directory
    let target_dir = temp.path().join("target");
    fs::create_dir_all(&git_dir)?; // Create .git as a directory
    fs::create_dir_all(&target_dir)?;

    // Create .gitignore
    let mut file = fs::File::create(gitignore_path)?;
    let gitignore_content = "target/\n*.log\n";
    write!(file, "{}", gitignore_content)?;
    drop(file);

    // Create files
    let src_content = "Source";
    let obj_content = "Object";
    let log_content = "Log";
    fs::write(temp.path().join("src.rs"), src_content)?;
    fs::write(target_dir.join("debug.o"), obj_content)?;
    fs::write(temp.path().join("app.log"), log_content)?;
    // Also write the .gitignore content itself to check if it's included with -t
    fs::write(temp.path().join(".gitignore"), gitignore_content)?;

    dircat_cmd()
        .arg("-t") // Ignore .gitignore AND hidden files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src.rs")) // Keep source header
        .stdout(predicate::str::contains(src_content)) // Keep source content
        .stdout(predicate::str::contains("## File: target/debug.o")) // Keep target/ header
        .stdout(predicate::str::contains(obj_content)) // Keep target/ content
        .stdout(predicate::str::contains("## File: app.log")) // Keep *.log header
        .stdout(predicate::str::contains(log_content)) // Keep *.log content
        // With -t, hidden files like .gitignore should be included
        .stdout(predicate::str::contains("## File: .gitignore")) // Keep .gitignore header
        .stdout(predicate::str::contains(gitignore_content)); // Keep .gitignore content

    temp.close()?;
    Ok(())
}

#[test]
fn test_respects_hidden_files_by_default() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let hidden_content = "Hidden";
    let visible_content = "Visible";
    fs::write(temp.path().join(".hidden.txt"), hidden_content)?;
    fs::write(temp.path().join("visible.txt"), visible_content)?;

    dircat_cmd()
        // No -t flag, should skip hidden files
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: visible.txt")) // Keep visible header
        .stdout(predicate::str::contains(visible_content)) // Keep visible content
        .stdout(predicate::str::contains("## File: .hidden.txt").not()) // Ignore hidden header
        .stdout(predicate::str::contains(hidden_content).not()); // Ignore hidden content

    temp.close()?;
    Ok(())
}

#[test]
fn test_no_gitignore_includes_hidden_files() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let hidden_content = "Hidden";
    let visible_content = "Visible";
    fs::write(temp.path().join(".hidden.txt"), hidden_content)?;
    fs::write(temp.path().join("visible.txt"), visible_content)?;

    dircat_cmd()
        .arg("-t") // -t should also disable the default hidden file filter
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: visible.txt")) // Keep visible header
        .stdout(predicate::str::contains(visible_content)) // Keep visible content
        .stdout(predicate::str::contains("## File: .hidden.txt")) // Should include hidden header
        .stdout(predicate::str::contains(hidden_content)); // Should include hidden content

    temp.close()?;
    Ok(())
}
