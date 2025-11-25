#![cfg(all(feature = "web", feature = "git"))]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router_with_config, ScanRequest};
use dircat::security::SafeModeConfig;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_web_scan_ignores_local_submodules() {
    // 1. Setup: Create a "Secret" outside the repo
    let temp = tempdir().unwrap();
    let secret_path = temp.path().join("secret.txt");
    fs::write(&secret_path, "SUPER_SECRET_DATA").unwrap();

    // 2. Setup: Create a Git Repo
    let repo_dir = temp.path().join("repo");
    fs::create_dir(&repo_dir).unwrap();
    let repo = git2::Repository::init(&repo_dir).unwrap();
    
    // Create a .gitmodules file pointing to the local secret
    // Note: In a real attack, this would be a valid git repo url, 
    // but pointing to file:// is the specific vector we want to block.
    let modules_content = format!(
        "[submodule \"secret\"]\n\tpath = secret\n\turl = file://{}\n", 
        secret_path.to_str().unwrap().replace('\\', "/")
    );
    fs::write(repo_dir.join(".gitmodules"), modules_content).unwrap();

    // Commit it
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new(".gitmodules")).unwrap();
    let oid = index.write_tree().unwrap();
    let signature = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree = repo.find_tree(oid).unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "Add malicious submodule", &tree, &[]).unwrap();

    // 3. Configure Safe Mode
    let mut config = SafeModeConfig::strict();
    // We must allow local paths for the *input* repo to be scanned, 
    // but we want to ensure the *submodule* (which is also local) is not followed/cloned.
    config.allow_local_paths = true; 

    let app = create_router_with_config(config);

    // 4. Request Scan
    let payload = ScanRequest {
        input_path: repo_dir.to_str().unwrap().to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: None,
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
    let json_str = String::from_utf8_lossy(&body);
    
    // 5. Assert: The secret data must NOT be in the output
    // dircat might list .gitmodules, but it should NOT have cloned the submodule content
    assert!(!json_str.contains("SUPER_SECRET_DATA"), "Security Fail: Submodule content leaked!");
}
