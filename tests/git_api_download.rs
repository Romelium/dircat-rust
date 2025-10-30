
mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;

/// Tests downloading a specific folder from a public remote repository via the API.
/// This is a slow, network-dependent test.
/// To run: `cargo test -- --ignored git_api_download`
#[test]
#[ignore = "requires network access and is slow"]
fn test_api_download_public_directory() -> Result<(), Box<dyn std::error::Error>> {
    // This public repo has a `go/` directory containing `example.go`.
    let repo_folder_url = "https://github.com/git-fixtures/basic/tree/master/go";

    dircat_cmd()
        .arg(repo_folder_url)
        .assert()
        .success()
        // Check for file inside the 'go' directory
        .stdout(predicate::str::contains("## File: example.go"))
        .stdout(predicate::str::contains("package harvesterd"))
        // Check that files from the root directory are NOT included
        .stdout(predicate::str::contains("## File: CHANGELOG").not())
        .stdout(predicate::str::contains("## File: LICENSE").not());

    Ok(())
}

/// Tests that the URL parser correctly rejects a path that contains a reserved
/// GitHub keyword (like 'issues', 'pull', 'releases') as its first segment.
/// This is a known limitation we are asserting.
#[test]
#[ignore = "requires network access and is slow"]
fn test_sloppy_url_with_reserved_keyword_is_not_parsed_as_folder() -> Result<(), Box<dyn std::error::Error>> {
    // This "sloppy" URL (missing /tree/) should be rejected by the folder parser because 'releases' is a reserved keyword.
    // dircat will then treat it as a generic git URL and attempt to clone it, which will fail.
    let repo_folder_url = "https://github.com/some-user/some-repo/releases/v1.0";

    dircat_cmd()
        .arg(repo_folder_url)
        .assert()
        .failure()
        // The error should indicate it failed to clone, because the folder parser rejected it
        // and it was then treated as a generic (but invalid) clone URL.
        .stderr(predicate::str::contains("Failed to clone repository"));

    Ok(())
}
