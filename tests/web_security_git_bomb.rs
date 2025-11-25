#![cfg(all(feature = "web", feature = "git"))]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::create_router_with_config;
use dircat::security::SafeModeConfig;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_aborts_clone_exceeding_size_limit() {
    // 1. Create a "Large" Repo (simulated)
    // We create a local repo with a file larger than our configured limit.
    let temp = tempdir().unwrap();
    let repo_dir = temp.path().join("large_repo");
    fs::create_dir(&repo_dir).unwrap();
    
    let repo = git2::Repository::init(&repo_dir).unwrap();
    
    // Create a 1MB file
    let large_data = vec![0u8; 1024 * 1024]; 
    fs::write(repo_dir.join("large.bin"), &large_data).unwrap();
    
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("large.bin")).unwrap();
    index.write().unwrap();
    let oid = index.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "Add large file", &tree, &[]).unwrap();

    // 2. Configure Safe Mode with a tiny limit (e.g., 1KB)
    let mut config = SafeModeConfig::strict();
    config.max_repo_size = 1024; // 1KB limit
    config.allow_local_paths = true; // Allow scanning our local test repo

    let app = create_router_with_config(config);

    // 3. Request Scan
    let payload = serde_json::json!({
        "input_path": repo_dir.to_str().unwrap(),
        "no_gitignore": false, "no_lockfiles": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Assert Failure
    // The server should return 400 Bad Request with a specific error message
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    let body = http_body_util::BodyExt::collect(response.into_body()).await.unwrap().to_bytes();
    let msg = String::from_utf8_lossy(&body);
    
    assert!(msg.contains("Repository size"), "Error message should mention size limit. Got: {}", msg);
    assert!(msg.contains("exceeds limit"), "Error message should mention limit exceeded. Got: {}", msg);
}
