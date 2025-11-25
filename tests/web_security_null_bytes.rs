#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_rejects_null_bytes_in_path() {
    let app = create_router();

    let payload = ScanRequest {
        // Attempt to trick logic by terminating string early
        input_path: "/etc/passwd\0.git".to_string(),
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

    // Should be Bad Request (due to validation or IO error), not Internal Server Error
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
