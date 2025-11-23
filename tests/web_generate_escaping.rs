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

/// Helper to send a generate request
async fn send_generate_request(
    app: axum::Router,
    input_path: &str,
    selected_files: Vec<String>,
) -> String {
    let payload = GenerateRequest {
        input_path: input_path.to_string(),
        selected_files: Some(selected_files),
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
    String::from_utf8_lossy(&body).to_string()
}

#[tokio::test]
async fn test_web_generate_handles_brackets_in_filenames() {
    let temp = tempdir().unwrap();

    // Scenario: File has brackets, which are glob meta-characters for character classes.
    // If treated as a glob, "data[v1].txt" looks for "datav.txt" or "data1.txt".
    let filename = "data[v1].txt";
    let content = "Important Data Content";
    fs::write(temp.path().join(filename), content).unwrap();

    let app = create_router();
    let output = send_generate_request(
        app,
        temp.path().to_str().unwrap(),
        vec![filename.to_string()],
    )
    .await;

    assert!(
        output.contains(&format!("## File: {}", filename)),
        "Failed to include file with brackets. Backend likely treated '{}' as a glob pattern.\nOutput: {}", 
        filename, output
    );
}

#[tokio::test]
async fn test_web_generate_handles_braces_in_filenames() {
    let temp = tempdir().unwrap();

    // Scenario: File has braces, which are glob meta-characters for grouping/alternation.
    // If treated as a glob, "image_{a,b}.txt" looks for "image_a.txt" or "image_b.txt".
    let filename = "image_{a,b}.txt";
    let content = "Image Content";
    fs::write(temp.path().join(filename), content).unwrap();

    let app = create_router();
    let output = send_generate_request(
        app,
        temp.path().to_str().unwrap(),
        vec![filename.to_string()],
    )
    .await;

    assert!(
        output.contains(&format!("## File: {}", filename)),
        "Failed to include file with braces. Backend likely treated '{}' as a glob pattern.\nOutput: {}", 
        filename, output
    );
}

#[tokio::test]
#[cfg(unix)] // Windows does not allow '*' in filenames
async fn test_web_generate_handles_literal_wildcards_security() {
    let temp = tempdir().unwrap();

    // Scenario: User has a file literally named "*.txt" (valid on Unix).
    // They also have "secret.txt".
    // They select ONLY "*.txt" in the UI.
    // If the backend treats "*.txt" as a glob, it will match "secret.txt" too.

    let literal_wildcard_name = "*.txt";
    let secret_name = "secret.txt";

    fs::write(
        temp.path().join(literal_wildcard_name),
        "I am a literal star file",
    )
    .unwrap();
    fs::write(temp.path().join(secret_name), "I am a secret").unwrap();

    let app = create_router();
    let output = send_generate_request(
        app,
        temp.path().to_str().unwrap(),
        vec![literal_wildcard_name.to_string()],
    )
    .await;

    // 1. The literal file MUST be present
    assert!(
        output.contains("## File: *.txt"),
        "Failed to include the file literally named '*.txt'"
    );

    // 2. The secret file MUST NOT be present
    // If this fails, it means the backend treated the input as a glob and expanded it,
    // leaking files the user did not explicitly select.
    assert!(
        !output.contains("## File: secret.txt"),
        "SECURITY FAIL: Selecting '*.txt' (literal) expanded to include 'secret.txt'. Input was treated as a glob."
    );
}
