// tests/content_processing.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_remove_comments() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_content = r#"
fn main() { // Line comment
    let x = 1; /* Block
                  Comment */
    let s = "// Not a comment";
    println!("{}", x); // Another comment
}
"#;
    fs::write(temp.path().join("code.rs"), file_content)?;

    // Define the expected content *after trimming* by remove_comments
    let expected_content = r#"fn main() {
    let x = 1;
    let s = "// Not a comment";
    println!("{}", x);
}"#; // Note: No leading/trailing newline

    // Construct the full expected block including fences and the expected content
    // write_file_block adds a newline after the last line of content before the fence.
    let full_expected_block = format!("```rs\n{}\n```", expected_content); // Adds \n before and after content

    dircat_cmd()
        .arg("-c") // Remove comments
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: code.rs"))
        // Assert that the stdout contains the fully constructed expected block
        .stdout(predicate::str::contains(&full_expected_block));

    temp.close()?;
    Ok(())
}

#[test]
fn test_remove_comments_and_empty_lines() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempdir()?;
    let file_content = r#"
// Header comment

fn main() { /* Block */

    let x = 1; // Assignment

} // End
"#;
    fs::write(temp.path().join("code.rs"), file_content)?;

    // 1. After comment removal (and trimming):
    //    Input: "\n// Header comment\n\nfn main() { /* Block */\n\n    let x = 1; // Assignment\n\n} // End\n"
    //    Result of remove_comments: "\n\nfn main() {\n\n    let x = 1;\n\n}\n"
    //    Result after trim(): "fn main() {\n\n    let x = 1;\n\n}"

    // 2. After empty line removal (applied to the result of step 1):
    //    Input to remove_empty_lines: "fn main() {\n\n    let x = 1;\n\n}"
    //    Lines: ["fn main() {", "", "    let x = 1;", "", "}"]
    //    Filtered lines: ["fn main() {", "    let x = 1;", "}"]
    //    Joined: "fn main() {\n    let x = 1;\n}"
    let expected_content = r#"fn main() {
    let x = 1;
}"#;

    // Construct the full expected block
    let full_expected_block = format!("```rs\n{}\n```", expected_content);

    dircat_cmd()
        .arg("-c") // Remove comments
        .arg("-l") // Remove empty lines
        .current_dir(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("## File: code.rs"))
        // Assert that the stdout contains the fully constructed expected block
        .stdout(predicate::str::contains(&full_expected_block));

    temp.close()?;
    Ok(())
}
