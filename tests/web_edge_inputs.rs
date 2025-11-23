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
async fn test_web_scan_invalid_regex_returns_400() {
    let temp = tempdir().unwrap();
    let app = create_router();

    // "[unclosed" is an invalid regex
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: None,
        git_depth: None,
        path_regex: Some("[unclosed_class".to_string()),
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error_msg = String::from_utf8_lossy(&body);
    assert!(error_msg.contains("Invalid path regex"));
}

#[tokio::test]
async fn test_web_scan_invalid_max_size_returns_400() {
    let temp = tempdir().unwrap();
    let app = create_router();

    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: Some("not_a_number".to_string()),
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error_msg = String::from_utf8_lossy(&body);
    assert!(error_msg.contains("Invalid size format"));
}

#[tokio::test]
async fn test_web_scan_whitespace_parsing_in_extensions() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.rs"), "rust").unwrap();
    fs::write(temp.path().join("b.txt"), "text").unwrap();
    fs::write(temp.path().join("c.md"), "markdown").unwrap();

    let app = create_router();

    // Test weird whitespace handling in the extension string
    // Should parse as ["rs", "md"]
    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: None,
        extensions: Some("  rs \n\t md  ".to_string()),
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

    assert!(paths.contains(&"a.rs"));
    assert!(paths.contains(&"c.md"));
    assert!(!paths.contains(&"b.txt"));
}

#[tokio::test]
async fn test_web_scan_multiline_regex_parsing() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test_a.rs"), "").unwrap();
    fs::write(temp.path().join("spec_b.rs"), "").unwrap();
    fs::write(temp.path().join("other.rs"), "").unwrap();

    let app = create_router();

    // Test multiline string parsing for regexes
    // Should match "test_.*" OR "spec_.*"
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
        filename_regex: Some("^test_.*\n^spec_.*".to_string()),
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

    assert!(paths.contains(&"test_a.rs"));
    assert!(paths.contains(&"spec_b.rs"));
    assert!(!paths.contains(&"other.rs"));
}
