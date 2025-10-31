// tests/process_order_adv.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

/// Tests that the `--only` shorthand correctly overrides a .gitignore rule.
#[test]
fn test_only_overrides_gitignore() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    fs::write(temp.path().join("important.log"), "IMPORTANT LOG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // Use --only to include ONLY the ignored file.
    dircat_cmd()
        .arg("-O") // --only alias
        .arg("*.log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: important.log"))
        .stdout(predicate::str::contains("IMPORTANT LOG"))
        .stdout(predicate::str::contains("## File: main.rs").not());

    Ok(())
}

/// Tests that `--last` still correctly orders files when gitignore processing is disabled with `-t`.
#[test]
fn test_last_with_no_gitignore_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    // This .gitignore would normally be ignored by -t anyway.
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    fs::write(temp.path().join("app.log"), "LOG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    dircat_cmd()
        .arg("-t") // Disable .gitignore
        .arg("-z") // Process .log files last
        .arg("*.log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            // Both files should be present because of -t
            let pos_main = output.find("## File: main.rs").unwrap();
            let pos_log = output.find("## File: app.log").unwrap();
            // The log file should appear after the main file
            pos_main < pos_log
        }));

    Ok(())
}

/// Tests that providing an invalid glob to --last doesn't break the command
/// and that the valid globs still work.
#[test]
fn test_last_with_invalid_glob_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(temp.path().join("b.rs"), "B")?;
    fs::write(temp.path().join("c.md"), "C")?;

    // The `ignore` crate will silently ignore the invalid glob pattern.
    // We just need to ensure the valid one still works as expected.
    dircat_cmd()
        .arg("-z")
        .arg("[invalid-glob") // This will be ignored by the parser
        .arg("-z")
        .arg("*.md") // This one is valid
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            let pos_a = output.find("## File: a.txt").unwrap();
            let pos_b = output.find("## File: b.rs").unwrap();
            let pos_c = output.find("## File: c.md").unwrap();
            // a.txt and b.rs should come before c.md
            pos_a < pos_c && pos_b < pos_c
        }));

    Ok(())
}

/// Tests that --last works correctly with filenames that contain spaces.
#[test]
fn test_last_with_filename_containing_spaces() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(temp.path().join("file with spaces.md"), "SPACES")?;

    dircat_cmd()
        .arg("-z")
        .arg("file with spaces.md")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            let pos_a = output.find("## File: a.txt").unwrap();
            let pos_spaces = output.find("## File: file with spaces.md").unwrap();
            pos_a < pos_spaces
        }));

    Ok(())
}

/// Tests a complex precedence case: a --last override should apply to files
/// ignored by a root .gitignore, even if a nested .gitignore has its own rules.
#[test]
fn test_last_overrides_root_gitignore_and_includes_all_matches(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;

    // Root .gitignore ignores all logs
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    // Nested .gitignore re-includes a specific log file
    let mut sub_gitignore = fs::File::create(sub.join(".gitignore"))?;
    writeln!(sub_gitignore, "!important.log")?;
    drop(sub_gitignore);

    fs::write(temp.path().join("a.log"), "A LOG")?;
    fs::write(sub.join("b.log"), "B LOG")?;
    fs::write(sub.join("important.log"), "IMPORTANT LOG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // We want to process ALL .log files last. This should override the root
    // .gitignore for a.log and b.log. important.log is already whitelisted
    // by the nested gitignore, but should also be grouped with the --last files.
    dircat_cmd()
        .arg("-z")
        .arg("*.log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: main.rs"))
        .stdout(predicate::str::contains("## File: a.log"))
        .stdout(predicate::str::contains("## File: sub/b.log"))
        .stdout(predicate::str::contains("## File: sub/important.log"))
        .stdout(predicate::function(|output: &str| {
            let pos_main = output.find("## File: main.rs").unwrap();
            let pos_a_log = output.find("## File: a.log").unwrap();
            let pos_b_log = output.find("## File: sub/b.log").unwrap();
            let pos_imp_log = output.find("## File: sub/important.log").unwrap();

            // The normal file must come before all --last files.
            pos_main < pos_a_log && pos_main < pos_b_log && pos_main < pos_imp_log
        }));

    Ok(())
}
