mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

/// Tests that the comment remover is smart enough to ignore comment-like
/// sequences that appear inside string literals, such as in URLs.
#[test]
fn test_remove_comments_preserves_urls_in_strings() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_content = r#"
const API_ENDPOINT: &str = "https://example.com/api/v1"; // Use secure protocol
// Another comment here.
var url = "http://insecure.com"; /* Please update */
"#;
    fs::write(temp.path().join("config.rs"), file_content)?;

    // The expected content after comment removal and trimming.
    // The URLs should be perfectly preserved.
    let expected_content = r#"const API_ENDPOINT: &str = "https://example.com/api/v1";

var url = "http://insecure.com";"#;

    let full_expected_block = format!("```rs\n{}\n```", expected_content);

    dircat_cmd()
        .arg("-c") // Remove comments
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: config.rs"))
        .stdout(predicate::str::contains(&full_expected_block));

    temp.close()?;
    Ok(())
}
