mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_only_flag_should_match_recursively() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    fs::create_dir(&src)?;
    fs::write(src.join("main.rs"), "fn main() {}")?;

    dircat_cmd()
        .arg("--only")
        .arg("*.rs")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: src/main.rs"));

    temp.close()?;
    Ok(())
}

#[test]
fn test_last_flag_should_match_recursively() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let src = temp.path().join("src");
    fs::create_dir(&src)?;

    // a.rs (in root)
    // src/b.rs (in subdir)
    // c.txt (in root)
    fs::write(temp.path().join("a.rs"), "A")?;
    fs::write(src.join("b.rs"), "B")?;
    fs::write(temp.path().join("c.txt"), "C")?;

    dircat_cmd()
        .arg("--last")
        .arg("*.rs")
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::function(|output: &str| {
            let pos_a = output.find("## File: a.rs").unwrap_or(usize::MAX);
            let pos_b = output.find("## File: src/b.rs").unwrap_or(usize::MAX);
            let pos_c = output.find("## File: c.txt").unwrap_or(usize::MAX);

            // We want to assert that b is treated as last, so it should come after c.
            // And since a is also last, a and b should be grouped at the end.
            pos_c < pos_b && pos_c < pos_a
        }));

    temp.close()?;
    Ok(())
}
