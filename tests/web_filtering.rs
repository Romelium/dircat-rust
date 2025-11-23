#![cfg(feature = "web")]

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

#[tokio::test]
async fn test_web_scan_extensions_filter() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("main.rs"), "fn main() {}").unwrap();
    fs::write(temp.path().join("readme.txt"), "Read me").unwrap();
    fs::write(temp.path().join("config.toml"), "[config]").unwrap();

    let app = create_router();

    // Filter for "rs" and "toml"
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: Some("rs toml".to_string()), // Space separated in UI
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
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();

    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    assert!(paths.contains(&"main.rs"));
    assert!(paths.contains(&"config.toml"));
    assert!(!paths.contains(&"readme.txt"));
}

#[tokio::test]
async fn test_web_scan_gitignore_toggle() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("visible.txt"), "Visible").unwrap();
    fs::write(temp.path().join("ignored.log"), "Log").unwrap();
    fs::write(temp.path().join(".gitignore"), "*.log").unwrap();

    let app = create_router();

    // Enable no_gitignore (ignore the gitignore file)
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: true, // <--- Key flag
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

    assert!(paths.contains(&"visible.txt"));
    assert!(paths.contains(&"ignored.log")); // Should be present because no_gitignore is true
}
