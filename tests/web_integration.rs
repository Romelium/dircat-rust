#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, GenerateRequest, ScanRequest};
use http_body_util::BodyExt; // Requires http-body-util in dev-dependencies
use serde_json::Value;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt; // for oneshot

#[tokio::test]
async fn test_web_static_index() {
    let app = create_router();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("<title>Dircat Studio</title>"));
}

#[tokio::test]
async fn test_web_api_scan() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "Content A").unwrap();
    fs::write(temp.path().join("b.rs"), "fn main() {}").unwrap();
    fs::create_dir(temp.path().join("sub")).unwrap();
    fs::write(temp.path().join("sub/c.md"), "# Markdown").unwrap();

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

    // Verify we found the 3 files
    assert_eq!(files.len(), 3);

    let paths: Vec<&str> = files
        .iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();

    // Order isn't guaranteed by scan, so check containment
    assert!(paths.contains(&"a.txt"));
    assert!(paths.contains(&"b.rs"));
    // Handle potential OS separator differences in test assertion if needed,
    // but dircat usually normalizes to some extent or uses OS separator.
    // The test runs on the same OS so it should match.
    assert!(paths.contains(&"sub/c.md") || paths.contains(&"sub\\c.md"));
}

#[tokio::test]
async fn test_web_api_generate() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "Content A").unwrap();
    fs::write(temp.path().join("ignored.txt"), "Should not be here").unwrap();

    let app = create_router();

    // We only select "a.txt"
    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: Some(vec!["a.txt".to_string()]),
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
    assert!(!output.contains("ignored.txt"));
}

#[tokio::test]
async fn test_web_api_404() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/non_existent_endpoint")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Our fallback is static_handler which returns 404 for unknown assets
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
