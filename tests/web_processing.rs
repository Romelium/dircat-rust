#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, GenerateRequest};
use http_body_util::BodyExt;
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_generate_remove_comments() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("code.rs"), "fn main() { // comment \n}").unwrap();

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,  // Process all
        remove_comments: true, // <--- Key flag
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

    assert!(output.contains("fn main() {"));
    assert!(!output.contains("// comment"));
}

#[tokio::test]
async fn test_web_generate_formatting_options() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.py"), "print('hello')").unwrap();

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,
        remove_comments: false,
        remove_empty_lines: false,
        include_binary: false,
        line_numbers: true,  // <---
        filename_only: true, // <---
        backticks: true,     // <---
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

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let output = String::from_utf8_lossy(&body);

    // Check filename only + backticks
    // Should look like: ## File: `test.py`
    assert!(output.contains("## File: `test.py`"));
    assert!(!output.contains("## File: `test.py` (")); // No extra path info

    // Check line numbers
    // Should look like: 1 | print('hello')
    assert!(output.contains(" 1 | print('hello')"));
}
