#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::security::SafeModeConfig;
use dircat::web::{create_router_with_config, ScanRequest};
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_rejects_massive_regex() {
    // We must enable Safe Mode for complexity limits to be enforced.
    let mut config = SafeModeConfig::default();
    config.enabled = true;
    let app = create_router_with_config(config);

    // Create a 10KB regex string
    let massive_regex = "a".repeat(10_000);

    let payload = ScanRequest {
        input_path: "https://github.com/user/repo".to_string(),
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: None,
        git_depth: None,
        path_regex: Some(massive_regex), // <--- Attack vector
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

    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("exceeds maximum length"));
}
