#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, CopyRequest, GenerateRequest, ScanRequest};
use http_body_util::BodyExt;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_max_size_filtering() {
    let temp = tempdir().unwrap();
    // Create a small file (5 bytes)
    fs::write(temp.path().join("small.txt"), "12345").unwrap();
    // Create a larger file (20 bytes)
    fs::write(temp.path().join("large.txt"), "12345678901234567890").unwrap();

    let app = create_router();

    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        max_size: Some("10".to_string()), // Limit to 10 bytes
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
    let files: Vec<Value> = serde_json::from_slice(&body).unwrap();

    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    assert!(paths.contains(&"small.txt"));
    assert!(!paths.contains(&"large.txt"));
}

#[tokio::test]
async fn test_web_generate_multi_file_selection() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "Content A").unwrap();
    fs::write(temp.path().join("b.txt"), "Content B").unwrap();
    fs::write(temp.path().join("c.txt"), "Content C").unwrap();

    let app = create_router();

    // Select A and C, ignore B
    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: Some(vec!["a.txt".to_string(), "c.txt".to_string()]),
        remove_comments: false,
        remove_empty_lines: false,
        include_binary: false,
        line_numbers: false,
        filename_only: false,
        backticks: false,
        git_branch: None,
        summary: false,
        counts: false,
        ticks: None,
        process_last: None,
        output_file: None,
        request_id: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/generate")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let output = String::from_utf8_lossy(&body);

    assert!(output.contains("## File: a.txt"));
    assert!(output.contains("Content A"));
    assert!(output.contains("## File: c.txt"));
    assert!(output.contains("Content C"));

    // B should not be present
    assert!(!output.contains("## File: b.txt"));
    assert!(!output.contains("Content B"));
}

#[tokio::test]
async fn test_web_scan_invalid_path_returns_error() {
    let app = create_router();

    let payload = ScanRequest {
        input_path: "/path/that/definitely/does/not/exist/on/this/system".to_string(),
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

    // The handler catches the error from resolve_input and returns 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error_msg = String::from_utf8_lossy(&body);
    assert!(error_msg.contains("Failed to resolve input path"));
}

#[tokio::test]
async fn test_web_copy_endpoint_structure() {
    // This test verifies the endpoint accepts the request structure.
    // It does NOT verify actual clipboard modification because that often fails
    // in headless CI environments (returning 500).

    let app = create_router();
    let payload = CopyRequest {
        content: "Test clipboard content".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copy")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // We accept either OK (clipboard worked) or INTERNAL_SERVER_ERROR (clipboard init failed).
    // We mostly want to ensure it didn't 404 or 400 (Bad Request).
    assert!(
        response.status() == StatusCode::OK
            || response.status() == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_web_request_timeout() {
    // Start server with 0ms timeout
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Use custom config to allow local paths (avoiding network hangs) but enforce 0ms timeout
    let mut config = dircat::security::SafeModeConfig::strict();
    config.request_timeout = std::time::Duration::from_millis(0);
    config.allow_local_paths = true;

    let app = dircat::web::create_router_with_config(config);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = format!("http://127.0.0.1:{}", port);

    // Create a dummy local file
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("a.txt");
    fs::write(&file_path, "content").unwrap();

    let client = reqwest::Client::new();
    // Trigger a scan that would take > 0ms
    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": temp.path().to_str().unwrap(),
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::REQUEST_TIMEOUT);
}
