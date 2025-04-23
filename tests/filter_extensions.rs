// tests/filter_extensions.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_extension_filter_include() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.txt"), "Text")?;
    fs::write(temp.path().join("b.rs"), "Rust")?;
    fs::write(temp.path().join("c.TXT"), "Upper Text")?; // Case check

    dircat_cmd()
        .arg("-e")
        .arg("txt") // Include only .txt
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: a.txt"))
        .stdout(predicate::str::contains("Text"))
        .stdout(predicate::str::contains("## File: c.TXT")) // Should match case-insensitively
        .stdout(predicate::str::contains("Upper Text"))
        .stdout(predicate::str::contains("## File: b.rs").not())
        .stdout(predicate::str::contains("Rust").not());

    temp.close()?;
    Ok(())
}

#[test]
fn test_extension_filter_exclude() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    fs::write(temp.path().join("a.log"), "Log")?;
    fs::write(temp.path().join("b.txt"), "Text")?;
    fs::write(temp.path().join("c.LOG"), "Upper Log")?; // Case check

    dircat_cmd()
        .arg("-x")
        .arg("log") // Exclude .log
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: b.txt"))
        .stdout(predicate::str::contains("Text"))
        .stdout(predicate::str::contains("## File: a.log").not())
        .stdout(predicate::str::contains("Log").not())
        .stdout(predicate::str::contains("## File: c.LOG").not()) // Should exclude case-insensitively
        .stdout(predicate::str::contains("Upper Log").not());

    temp.close()?;
    Ok(())
}
