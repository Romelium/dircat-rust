
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

/// Tests that the --git-branch flag correctly overrides the branch in the URL
/// when using the API downloader.
/// To run: `cargo test -- --ignored git_api_download`
#[test]
#[ignore = "requires network access and is slow"]
fn test_api_download_branch_override() -> Result<(), Box<dyn std::error::Error>> {
    // The URL points to the 'main' branch of dircat-rust itself.
    let repo_folder_url = "https://github.com/romelium/dircat-rust/tree/main/src/discovery";

    // However, we will check out a specific, older commit tag where a file had different content.
    let older_tag = "v0.5.1";
    let expected_content = "last_files.sort_by_key(|fi| fi.process_last_order);";

    dircat_cmd()
        .arg(repo_folder_url)
        .arg("--git-branch")
        .arg(older_tag)
        .assert()
        .success()
        // Check that we have the content from the older tag, not from 'main'
        .stdout(predicate::str::contains("## File: mod.rs"))
        .stdout(predicate::str::contains(expected_content))
        // Verify we DON'T have the content from 'main'
        .stdout(predicate::str::contains("last_files.sort_by_key(|fi| (fi.process_last_order, fi.relative_path.clone()));").not());

    Ok(())
}

/// Tests that the command fails gracefully when a non-existent directory is requested.
/// To run: `cargo test -- --ignored git_api_download`
#[test]
#[ignore = "requires network access and is slow"]
fn test_api_download_non_existent_directory() -> Result<(), Box<dyn std::error::Error>> {
    let repo_folder_url = "https://github.com/romelium/dircat-rust/tree/main/this-directory-does-not-exist";
    let api_error_msg = "Subdirectory 'this-directory-does-not-exist' not found in repository.";
    let fallback_error_msg = "Subdirectory 'this-directory-does-not-exist' not found in the cloned repository";

    dircat_cmd()
        .arg(repo_folder_url)
        .assert()
        .failure()
        // The test might fail via the API (404) or the clone fallback (if rate-limited).
        // We accept either error message.
        .stderr(predicate::str::contains(api_error_msg).or(predicate::str::contains(fallback_error_msg)));

    Ok(())
}

// Note: Testing private repositories requires a valid GITHUB_TOKEN with repo scope
// in the test environment. The logic is covered by the public repo tests, as the
// primary difference is the `Authorization` header, which is handled by `build_reqwest_client`.
// A manual test can be performed by setting the token and running:
// `cargo test -- --ignored git_api_download`

/// Tests downloading using a "sloppy" URL (without /tree/ in it).
/// To run: `cargo test -- --ignored git_api_download`
#[test]
#[ignore = "requires network access and is slow"]
fn test_api_download_sloppy_url() -> Result<(), Box<dyn std::error::Error>> {
    // This URL is missing the `/tree/master` part. The parser should infer the path.
    let repo_folder_url = "https://github.com/git-fixtures/basic/go";

    dircat_cmd()
        .arg(repo_folder_url)
        .assert()
        .success()
        // Check for file inside the 'go' directory
        .stdout(predicate::str::contains("## File: example.go"))
        .stdout(predicate::str::contains("package harvesterd"))
        // Check that files from the root directory are NOT included
        .stdout(predicate::str::contains("## File: CHANGELOG").not());

    Ok(())
}
