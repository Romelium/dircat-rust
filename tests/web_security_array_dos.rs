#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, GenerateRequest};
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_generate_rejects_massive_file_selection_array() {
    let app = create_router();

    // Create a payload with 1 million selected files
    // This tests if the server attempts to allocate a massive Vec<String>
    let huge_vec: Vec<String> = (0..1_000_000).map(|i| format!("file_{}.txt", i)).collect();

    let payload = GenerateRequest {
        input_path: ".".to_string(),
        selected_files: Some(huge_vec),
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

    // We need to serialize manually to ensure the body size is actually sent
    let body_content = serde_json::to_string(&payload).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/generate")
                .header("Content-Type", "application/json")
                .body(Body::from(body_content))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 413 Payload Too Large (if Axum default limits are active)
    // or 400 Bad Request. It should NOT crash the server.
    assert!(
        response.status() == StatusCode::PAYLOAD_TOO_LARGE
            || response.status() == StatusCode::BAD_REQUEST
    );
}
