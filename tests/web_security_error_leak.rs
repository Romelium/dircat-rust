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
async fn test_web_scan_sanitizes_internal_paths_in_errors() {
    let mut config = SafeModeConfig::strict();
    // Allow local paths so we can trigger a local file error
    config.allow_local_paths = true;
    let app = create_router_with_config(config);

    // Request a path that doesn't exist to trigger an IO error containing the path
    let secret_path = "/home/user/secret/project";

    let payload = ScanRequest {
        input_path: secret_path.to_string(),
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error_msg = String::from_utf8_lossy(&body);

    // The error should NOT contain the full secret path
    assert!(
        !error_msg.contains("/home/user/secret/project"),
        "Error message leaked internal path"
    );
    assert!(
        error_msg.contains("<path_redacted>"),
        "Error message was not sanitized"
    );
}
