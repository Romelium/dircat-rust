#![cfg(feature = "web")]

use axum::{body::Body, http::Request};
use dircat::web::{create_router, GenerateRequest};
use http_body_util::BodyExt;
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_generate_custom_ticks() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "content").unwrap();

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
        ticks: Some(5), // Request 5 backticks
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

    assert!(output.contains("`````txt")); // 5 ticks
    assert!(output.contains("`````\n"));
}

#[tokio::test]
async fn test_web_generate_include_binary() {
    let temp = tempdir().unwrap();
    let bin_path = temp.path().join("data.bin");
    let mut f = fs::File::create(&bin_path).unwrap();
    // Use an invalid UTF-8 byte (0xFF) to ensure lossy conversion occurs.
    // \0 is valid UTF-8, so it wouldn't trigger the replacement character check.
    f.write_all(b"binary\xFFdata").unwrap();

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,
        remove_comments: false,
        remove_empty_lines: false,
        include_binary: true, // Enable binary inclusion
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

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let output = String::from_utf8_lossy(&body);

    assert!(output.contains("## File: data.bin"));
    // Check for lossy representation of binary data (0xFF becomes U+FFFD)
    assert!(output.contains("binary\u{FFFD}data"));
}

#[tokio::test]
async fn test_web_generate_summary_and_counts() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "one\ntwo").unwrap();

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
        summary: true,
        counts: true, // Enable counts
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

    assert!(output.contains("Processed Files: (1)"));
    // Check for count format: (L:2 C:7 W:2) approx
    assert!(output.contains("a.txt (L:2"));
}
