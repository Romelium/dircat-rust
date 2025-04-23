// tests/filter_gitignore_advanced.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_gitignore_negation() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.log"), "Generic Log")?;
    fs::write(temp.path().join("important.log"), "Important Log")?;
    fs::write(temp.path().join("config.txt"), "Config")?;

    let mut gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(gitignore, "*.log")?; // Ignore all logs
    writeln!(gitignore, "!important.log")?; // But keep this one
    drop(gitignore);

    // Default behavior (respect .gitignore)
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: important.log")) // Keep
        .stdout(predicate::str::contains("Important Log"))
        .stdout(predicate::str::contains("## File: config.txt")) // Keep
        .stdout(predicate::str::contains("Config"))
        .stdout(predicate::str::contains("## File: a.log").not()) // Ignore
        .stdout(predicate::str::contains("Generic Log").not());

    // With -t (ignore .gitignore)
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: important.log")) // Keep
        .stdout(predicate::str::contains("## File: config.txt")) // Keep
        .stdout(predicate::str::contains("## File: a.log")); // Keep

    temp.close()?;
    Ok(())
}

#[test]
fn test_gitignore_anchoring() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("root.log"), "Root Log")?;
    fs::write(sub.join("root.log"), "Sub Log")?;
    fs::write(temp.path().join("other.txt"), "Other")?;

    let mut gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(gitignore, "/root.log")?; // Ignore only at the root
    drop(gitignore);

    // Default behavior
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/root.log")) // Keep sub
        .stdout(predicate::str::contains("Sub Log"))
        .stdout(predicate::str::contains("## File: other.txt")) // Keep other
        .stdout(predicate::str::contains("Other"))
        .stdout(predicate::str::contains("## File: root.log").not()) // Ignore root
        .stdout(predicate::str::contains("Root Log").not());

    // With -t
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/root.log")) // Keep
        .stdout(predicate::str::contains("## File: other.txt")) // Keep
        .stdout(predicate::str::contains("## File: root.log")); // Keep

    temp.close()?;
    Ok(())
}

#[test]
fn test_gitignore_directory_only() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let build_dir = temp.path().join("build");
    // Rename the FILE to avoid name collision on Windows
    let build_file_name = "build.file";
    fs::write(temp.path().join(build_file_name), "File named build")?;
    // Then create the DIRECTORY
    fs::create_dir(&build_dir)?;
    fs::write(build_dir.join("output.o"), "Object")?;

    let mut gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(gitignore, "build/")?; // Ignore the directory build/
    drop(gitignore);

    // Default behavior
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "## File: {}",
            build_file_name
        ))) // Keep the file
        .stdout(predicate::str::contains("File named build"))
        .stdout(predicate::str::contains("## File: build/output.o").not()) // Ignore dir content
        .stdout(predicate::str::contains("Object").not());

    // With -t
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "## File: {}",
            build_file_name
        ))) // Keep file
        .stdout(predicate::str::contains("## File: build/output.o")); // Keep dir content

    temp.close()?;
    Ok(())
}

#[test]
fn test_gitignore_recursive_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let a_dir = temp.path().join("a");
    let b_dir = a_dir.join("b");
    fs::create_dir_all(&b_dir)?;
    fs::write(temp.path().join("temp.txt"), "Root Temp")?;
    fs::write(a_dir.join("temp.txt"), "A Temp")?;
    fs::write(b_dir.join("temp.txt"), "B Temp")?;
    fs::write(temp.path().join("keep.txt"), "Keep")?;

    let mut gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(gitignore, "**/temp.txt")?; // Ignore temp.txt anywhere
    drop(gitignore);

    // Default behavior
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: keep.txt")) // Keep
        .stdout(predicate::str::contains("Keep"))
        .stdout(predicate::str::contains("## File: temp.txt").not()) // Ignore root
        .stdout(predicate::str::contains("## File: a/temp.txt").not()) // Ignore sub
        .stdout(predicate::str::contains("## File: a/b/temp.txt").not()); // Ignore nested

    // With -t
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: keep.txt")) // Keep
        .stdout(predicate::str::contains("## File: temp.txt")) // Keep
        .stdout(predicate::str::contains("## File: a/temp.txt")) // Keep
        .stdout(predicate::str::contains("## File: a/b/temp.txt")); // Keep

    temp.close()?;
    Ok(())
}

#[test]
fn test_gitignore_escaping() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("#config.yaml"), "Hash Config")?;
    fs::write(temp.path().join("config.yaml"), "Normal Config")?;

    let mut gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(gitignore, "\\#config.yaml")?; // Ignore literal #config.yaml
    drop(gitignore);

    // Default behavior
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: config.yaml")) // Keep normal
        .stdout(predicate::str::contains("Normal Config"))
        .stdout(predicate::str::contains("## File: #config.yaml").not()) // Ignore escaped
        .stdout(predicate::str::contains("Hash Config").not());

    // With -t
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: config.yaml")) // Keep
        .stdout(predicate::str::contains("## File: #config.yaml")); // Keep

    temp.close()?;
    Ok(())
}

#[test]
fn test_gitignore_nested() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub_dir = temp.path().join("sub");
    fs::create_dir(&sub_dir)?;

    fs::write(temp.path().join("a.log"), "Root Log")?;
    fs::write(sub_dir.join("b.log"), "Sub Log")?;
    fs::write(sub_dir.join("keep.txt"), "Keep Sub")?;
    fs::write(temp.path().join("root.txt"), "Keep Root")?;

    // Root .gitignore
    let mut root_gitignore = fs::File::create(temp.path().join(".gitignore"))?;
    writeln!(root_gitignore, "*.log")?; // Ignore all logs
    drop(root_gitignore);

    // Subdir .gitignore
    let mut sub_gitignore = fs::File::create(sub_dir.join(".gitignore"))?;
    writeln!(sub_gitignore, "!b.log")?; // Re-include b.log in this subdir
    writeln!(sub_gitignore, "keep.txt")?; // Ignore keep.txt in this subdir
    drop(sub_gitignore);

    // Default behavior
    dircat_cmd()
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/b.log")) // Re-included by sub gitignore
        .stdout(predicate::str::contains("Sub Log"))
        .stdout(predicate::str::contains("## File: root.txt")) // Keep root txt
        .stdout(predicate::str::contains("Keep Root"))
        .stdout(predicate::str::contains("## File: a.log").not()) // Ignored by root gitignore
        .stdout(predicate::str::contains("Root Log").not())
        .stdout(predicate::str::contains("## File: sub/keep.txt").not()) // Ignored by sub gitignore
        .stdout(predicate::str::contains("Keep Sub").not());

    // With -t
    dircat_cmd()
        .arg("-t")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/b.log")) // Keep
        .stdout(predicate::str::contains("## File: root.txt")) // Keep
        .stdout(predicate::str::contains("## File: a.log")) // Keep
        .stdout(predicate::str::contains("## File: sub/keep.txt")); // Keep

    temp.close()?;
    Ok(())
}
