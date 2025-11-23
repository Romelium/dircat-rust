#![cfg(all(feature = "web", feature = "git"))]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use http_body_util::BodyExt;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

// Helper to create a local git repo with two branches
fn setup_git_repo() -> (tempfile::TempDir, String) {
    let temp = tempdir().unwrap();
    let repo_path = temp.path();
    let repo = git2::Repository::init(repo_path).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();

    // Commit to main/master (default branch)
    fs::write(repo_path.join("main_file.txt"), "main").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("main_file.txt"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let commit_id = repo
        .commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
        .unwrap();
    let commit = repo.find_commit(commit_id).unwrap();

    // Capture the default branch name (could be master or main depending on git config)
    let default_branch = repo.head().unwrap().name().unwrap().to_string();

    // Create 'dev' branch and switch
    repo.branch("dev", &commit, false).unwrap();
    let obj = repo.revparse_single("refs/heads/dev").unwrap();
    repo.checkout_tree(&obj, None).unwrap();
    repo.set_head("refs/heads/dev").unwrap();

    // Commit to dev
    fs::write(repo_path.join("dev_file.txt"), "dev").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_path(std::path::Path::new("dev_file.txt"))
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Dev commit", &tree, &[&parent])
        .unwrap();

    // Switch back to the default branch so the working directory is in 'main' state
    let obj = repo.revparse_single(&default_branch).unwrap();
    repo.checkout_tree(&obj, None).unwrap();
    repo.set_head(&default_branch).unwrap();

    // Construct file:// URL
    #[cfg(windows)]
    let url = format!("file:///{}", repo_path.to_str().unwrap().replace('\\', "/"));
    #[cfg(not(windows))]
    let url = format!("file://{}", repo_path.to_str().unwrap());

    (temp, url)
}

#[tokio::test]
async fn test_web_scan_git_branch_parameter() {
    let (_repo_dir, repo_url) = setup_git_repo();
    let cache_dir = tempdir().unwrap();

    // We must set the cache dir env var because the web handler uses the standard resolution logic
    // which looks for DIRCAT_TEST_CACHE_DIR
    std::env::set_var("DIRCAT_TEST_CACHE_DIR", cache_dir.path());

    let app = create_router();

    // Request the 'dev' branch via the API
    let payload = ScanRequest {
        input_path: repo_url,
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: Some("dev".to_string()), // <--- Requesting specific branch
        git_depth: None,
        path_regex: None,
        filename_regex: None,
        exclude_path_regex: None,
        ignore_patterns: None,
        request_id: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();
    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    // The 'dev' branch has dev_file.txt.
    // If the branch param was ignored, we'd see the default branch (main/master) which lacks it.
    assert!(paths.contains(&"dev_file.txt"));
    assert!(paths.contains(&"main_file.txt")); // Inherited from parent commit

    std::env::remove_var("DIRCAT_TEST_CACHE_DIR");
}
