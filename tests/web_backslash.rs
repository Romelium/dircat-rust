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
#[cfg(unix)] // Backslashes are valid filename characters on Unix, making this test deterministic
async fn test_web_generate_handles_backslash_in_filename() {
    let temp = tempdir().unwrap();

    // 1. Create a file with a literal backslash in its name.
    // On Unix, this is a single file named "foo\bar.txt".
    let filename = "foo\\bar.txt";
    let content = "Content with backslash";
    fs::write(temp.path().join(filename), content).unwrap();

    let app = create_router();

    // 2. Select this file via the API.
    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: Some(vec![filename.to_string()]),
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

    // 3. Assert the file is present.
    assert!(
        output.contains("## File: foo\\bar.txt"),
        "Failed to include file with backslash. Backend likely failed to escape '\\'.\nOutput: {}",
        output
    );
}
