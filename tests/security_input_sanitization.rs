#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_rejects_malicious_branch_name() {
    let app = create_router();

    // Attempt command injection or shell expansion chars
    let malicious_branch = "master; rm -rf /";

    let payload = ScanRequest {
        input_path: "https://github.com/user/repo".to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: Some(malicious_branch.to_string()), // <--- Attack
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_web_scan_rejects_git_flags_in_branch() {
    let app = create_router();
    let malicious_branch = "--upload-pack=touch /tmp/pwned";

    let payload = ScanRequest {
        input_path: "https://github.com/user/repo".to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: Some(malicious_branch.to_string()),
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
