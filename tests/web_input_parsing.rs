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
async fn test_web_scan_empty_strings_treated_as_none() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "A").unwrap();

    let app = create_router();

    // Send empty strings for optional fields.
    // If treated as Some(""), regex compilation might fail or match everything/nothing unexpectedly.
    // The logic should treat them as None.
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: Some("   ".to_string()), // Whitespace only
        extensions: Some("".to_string()),
        exclude_extensions: Some("".to_string()),
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: Some("".to_string()),
        git_depth: None,
        path_regex: Some("".to_string()),
        filename_regex: Some("".to_string()),
        exclude_path_regex: Some("".to_string()),
        ignore_patterns: Some("".to_string()),
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

    // Should find the file, meaning filters were effectively disabled (None)
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["relative_path"], "a.txt");
}

#[tokio::test]
async fn test_web_scan_multiline_ignore_patterns() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.log"), "log").unwrap();
    fs::write(temp.path().join("b.tmp"), "tmp").unwrap();
    fs::write(temp.path().join("c.txt"), "txt").unwrap();

    let app = create_router();

    // Test that ignore_patterns splits by newline correctly
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
        ignore_patterns: Some("*.log\n   *.tmp   ".to_string()), // Multiline with whitespace
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

    assert!(paths.contains(&"c.txt"));
    assert!(!paths.contains(&"a.log"));
    assert!(!paths.contains(&"b.tmp"));
}
