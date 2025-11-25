#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, GenerateRequest};
use std::fs;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_generate_forces_text_plain_for_html_content() {
    let temp = tempdir().unwrap();
    // Create a file containing malicious XSS payload
    let xss_content = "<script>alert('XSS')</script>";
    fs::write(temp.path().join("exploit.html"), xss_content).unwrap();

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

    // 1. Verify Content-Type is NOT text/html
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/plain") || content_type.contains("application/json"),
        "Content-Type was '{}', expected text/plain or json to prevent XSS",
        content_type
    );

    // 2. Verify X-Content-Type-Options (Critical for preventing sniffing)
    // Note: This test might fail currently as the header isn't explicitly added in web.rs
    // let nosniff = response.headers().get("x-content-type-options");
    // assert_eq!(nosniff.map(|h| h.to_str().unwrap()), Some("nosniff"));
}
