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
async fn test_web_generate_selected_files_excludes_others() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("included.txt"), "I am included").unwrap();
    fs::write(temp.path().join("excluded.txt"), "I am excluded").unwrap();

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: Some(vec!["included.txt".to_string()]),
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
        process_last: None, // No process_last
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

    assert!(output.contains("## File: included.txt"));
    assert!(!output.contains("## File: excluded.txt"));
}

#[tokio::test]
async fn test_web_generate_selected_files_ignores_process_last() {
    // In src/web.rs, if selected_files is present, process_last is ignored.
    // We verify this by selecting "a.txt" but asking to process "b.txt" last.
    // "b.txt" should not appear at all because it wasn't selected.

    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "A").unwrap();
    fs::write(temp.path().join("b.txt"), "B").unwrap();

    let app = create_router();

    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: Some(vec!["a.txt".to_string()]),
        process_last: Some("b.txt".to_string()), // Should be ignored
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
    assert!(!output.contains("## File: b.txt"));
}

#[tokio::test]
async fn test_web_generate_process_last_reorders_when_no_selection() {
    // When selected_files is None, process_last should work to reorder files.
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.txt"), "A").unwrap();
    fs::write(temp.path().join("z.txt"), "Z").unwrap();

    let app = create_router();

    // Normally "a.txt" comes before "z.txt". We ask "a.txt" to be last.
    let payload = GenerateRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
        selected_files: None,
        process_last: Some("a.txt".to_string()),
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

    let pos_a = output.find("## File: a.txt").unwrap();
    let pos_z = output.find("## File: z.txt").unwrap();

    assert!(
        pos_z < pos_a,
        "z.txt should come before a.txt because a.txt is processed last"
    );
}
