#![cfg(feature = "web")]

use axum::{body::Body, http::Request};
use dircat::web::{create_router, ScanRequest};
use http_body_util::BodyExt;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_exclude_path_regex() {
    let temp = tempdir().unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::create_dir(temp.path().join("tests")).unwrap();
    fs::write(temp.path().join("src/main.rs"), "main").unwrap();
    fs::write(temp.path().join("tests/test.rs"), "test").unwrap();

    let app = create_router();

    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: None,
        git_depth: None,
        path_regex: None,
        filename_regex: None,
        exclude_path_regex: Some("^tests/".to_string()), // Exclude tests folder
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

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();
    // Normalize paths to forward slashes for cross-platform assertion
    let paths: Vec<String> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap().replace('\\', "/"))
        .collect();

    assert!(paths.contains(&"src/main.rs".to_string()));
    assert!(!paths.contains(&"tests/test.rs".to_string()));
}

#[tokio::test]
async fn test_web_scan_ignore_patterns_glob() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("keep.txt"), "keep").unwrap();
    fs::write(temp.path().join("temp.log"), "log").unwrap();
    fs::write(temp.path().join("secret.key"), "key").unwrap();

    let app = create_router();

    // Ignore logs and keys via custom glob patterns
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
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
        ignore_patterns: Some("*.log\n*.key".to_string()), // Multiline input
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

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();
    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    assert!(paths.contains(&"keep.txt"));
    assert!(!paths.contains(&"temp.log"));
    assert!(!paths.contains(&"secret.key"));
}

#[tokio::test]
async fn test_web_scan_exclude_extensions_overrides_include() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.rs"), "rust").unwrap();
    fs::write(temp.path().join("b.txt"), "text").unwrap();

    let app = create_router();

    // Include "rs" and "txt", but Exclude "txt"
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: Some("rs txt".to_string()),
        exclude_extensions: Some("txt".to_string()),
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

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();
    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    assert!(paths.contains(&"a.rs"));
    assert!(!paths.contains(&"b.txt"));
}
