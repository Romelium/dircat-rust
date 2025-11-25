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
async fn test_web_generate_creates_server_side_file() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "Content").unwrap();

    let output_dir = tempdir().unwrap();
    let output_file = output_dir.path().join("server_output.md");

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,
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
        output_file: Some(output_file.to_str().unwrap().to_string()), // <--- Requesting server-side write
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

    // Verify file was created on disk
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert!(content.contains("## File: a.txt"));
}

#[tokio::test]
async fn test_web_generate_invalid_output_path_returns_400() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "Content").unwrap();

    let app = create_router();

    // Try to write to a directory that doesn't exist
    let invalid_output = "/path/does/not/exist/output.md";

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,
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
        output_file: Some(invalid_output.to_string()),
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

    // Should fail because File::create fails.
    // The server maps application errors (like IO errors) to BAD_REQUEST (400).
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error_msg = String::from_utf8_lossy(&body);
    // The error message usually contains "No such file or directory" or similar OS error
    assert!(!error_msg.is_empty());
}

#[tokio::test]
async fn test_web_generate_enforces_max_output_size() {
    let temp = tempdir().unwrap();
    // Create 2MB file
    let content = "a".repeat(2 * 1024 * 1024);
    fs::write(temp.path().join("large.txt"), &content).unwrap();

    // Start server with 1MB output limit via custom router config
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut config = dircat::security::SafeModeConfig::strict();
        config.max_output_size = 1 * 1024 * 1024; // 1MB
        config.allow_local_paths = true;
        let app = dircat::web::create_router_with_config(config);
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/api/generate", port))
        .json(&serde_json::json!({
            "input_path": temp.path().to_str().unwrap(),
            "selected_files": None::<Vec<String>>,
            "remove_comments": false, "remove_empty_lines": false, "include_binary": false,
            "line_numbers": false, "filename_only": false, "backticks": false,
            "summary": false, "counts": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let text = resp.text().await.unwrap();
    assert!(text.contains("Output size limit exceeded"));
}
