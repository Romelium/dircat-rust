// tests/process_order.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_process_last_glob() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(sub.join("b.rs"), "B")?;
    fs::write(temp.path().join("last.md"), "LAST_MD")?;
    fs::write(sub.join("final.rs"), "FINAL_RS")?;

    // Process *.md and sub/*.rs last, in that order
    dircat_cmd()
        .arg("-z")
        .arg("*.md") // Match last.md
        .arg("-z")
        .arg("sub/*.rs") // Match sub/b.rs and sub/final.rs
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            // Find the positions of the file headers
            let pos_a = output.find("## File: a.txt").unwrap_or(usize::MAX);
            let pos_last_md = output.find("## File: last.md").unwrap_or(usize::MAX);
            // Note: sub/b.rs and sub/final.rs match the second glob.
            // Their relative order among themselves is alphabetical.
            let pos_sub_b = output.find("## File: sub/b.rs").unwrap_or(usize::MAX);
            let pos_sub_final = output.find("## File: sub/final.rs").unwrap_or(usize::MAX);

            // Check normal file comes first
            let normal_ok = pos_a < pos_last_md && pos_a < pos_sub_b && pos_a < pos_sub_final;

            // Check file matching first glob comes before files matching second glob
            let glob1_ok = pos_last_md < pos_sub_b && pos_last_md < pos_sub_final;

            // Check files matching second glob appear last, ordered alphabetically
            let glob2_order_ok = pos_sub_b < pos_sub_final;

            normal_ok && glob1_ok && glob2_order_ok
        }));

    temp.close()?;
    Ok(())
}

#[test]
fn test_only_shorthand() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(sub.join("b.rs"), "B")?;
    fs::write(temp.path().join("last.md"), "LAST")?;

    // Process only *.md
    dircat_cmd()
        .arg("--only")
        .arg("*.md")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: last.md"))
        .stdout(predicate::str::contains("LAST"))
        .stdout(predicate::str::contains("## File: a.txt").not())
        .stdout(predicate::str::contains("## File: sub/b.rs").not());

    Ok(())
}

#[test]
fn test_only_shorthand_multiple_args() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(sub.join("b.rs"), "B")?;
    fs::write(temp.path().join("last.md"), "LAST")?;

    // Process only *.md and sub/*.rs
    dircat_cmd()
        .arg("--only")
        .arg("*.md")
        .arg("--only")
        .arg("sub/*.rs")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: last.md"))
        .stdout(predicate::str::contains("## File: sub/b.rs"))
        .stdout(predicate::str::contains("## File: a.txt").not());

    Ok(())
}

#[test]
fn test_only_conflicts_with_last() -> Result<(), Box<dyn std::error::Error>> {
    dircat_cmd()
        .arg("--only")
        .arg("*.rs")
        .arg("--last")
        .arg("*.md")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the argument '--only <GLOB>...' cannot be used with '--last <GLOB>...'",
        ));

    Ok(())
}

#[test]
fn test_only_conflicts_with_only_last() -> Result<(), Box<dyn std::error::Error>> {
    dircat_cmd()
        .arg("--only")
        .arg("*.rs")
        .arg("--only-last")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the argument '--only <GLOB>...' cannot be used with '--only-last'",
        ));

    Ok(())
}

#[test]
fn test_only_shorthand_alias() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(temp.path().join("last.md"), "LAST")?;

    // Process only *.md using the -O alias
    dircat_cmd()
        .arg("-O")
        .arg("*.md")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: last.md"))
        .stdout(predicate::str::contains("LAST"))
        .stdout(predicate::str::contains("## File: a.txt").not());

    Ok(())
}

#[test]
fn test_only_last_glob() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;
    fs::write(temp.path().join("a.txt"), "A")?;
    fs::write(sub.join("b.rs"), "B")?;
    fs::write(temp.path().join("last.md"), "LAST")?;

    // Process only *.md
    dircat_cmd()
        .arg("-z")
        .arg("*.md")
        .arg("-Z") // Only last
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: last.md"))
        .stdout(predicate::str::contains("LAST"))
        .stdout(predicate::str::contains("## File: a.txt").not())
        .stdout(predicate::str::contains("## File: sub/b.rs").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_last_overrides_gitignore_and_keeps_others() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    fs::write(temp.path().join("important.log"), "IMPORTANT LOG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // Use --last to specifically request the ignored file.
    // It should be included at the end, and main.rs should be included normally.
    dircat_cmd()
        .arg("-z")
        .arg("*.log")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            let pos_main = output.find("## File: main.rs").unwrap_or(usize::MAX);
            let pos_log = output.find("## File: important.log").unwrap_or(usize::MAX);

            // Check that both files are present and the log file comes last.
            pos_main != usize::MAX && pos_log != usize::MAX && pos_main < pos_log
        }));

    Ok(())
}

#[test]
fn test_only_last_overrides_gitignore() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join(".gitignore"), "*.log")?;
    fs::write(temp.path().join("important.log"), "IMPORTANT LOG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // Use --last and --only-last to include only the ignored file.
    dircat_cmd()
        .arg("-z")
        .arg("*.log")
        .arg("-Z")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: important.log"))
        .stdout(predicate::str::contains("IMPORTANT LOG"))
        .stdout(predicate::str::contains("## File: main.rs").not());

    Ok(())
}

#[test]
fn test_last_overrides_nested_gitignore() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let sub = temp.path().join("sub");
    fs::create_dir(&sub)?;

    // Nested .gitignore ignores *.tmp in the subdirectory
    fs::write(sub.join(".gitignore"), "*.tmp")?;
    fs::write(sub.join("config.tmp"), "SECRET TMP")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // Use --last with a relative path to pull in the ignored file.
    dircat_cmd()
        .arg("-z")
        .arg("sub/config.tmp")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: sub/config.tmp"))
        .stdout(predicate::str::contains("SECRET TMP"))
        .stdout(predicate::str::contains("## File: main.rs"));

    Ok(())
}

#[test]
fn test_last_does_not_override_custom_ignore_i() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("config.toml"), "CONFIG")?;
    fs::write(temp.path().join("main.rs"), "fn main() {}")?;

    // Use -i to ignore *.toml, and -z to try to re-include it.
    // The -i filter should take precedence.
    dircat_cmd()
        .arg("-i")
        .arg("*.toml") // Custom ignore pattern
        .arg("-z")
        .arg("*.toml") // Try to process it last
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: main.rs"))
        .stdout(predicate::str::contains("## File: config.toml").not())
        .stdout(predicate::str::contains("CONFIG").not());

    Ok(())
}
