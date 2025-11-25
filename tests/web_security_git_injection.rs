#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::security::SafeModeConfig;
use dircat::web::{create_router_with_config, ScanRequest};
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_blocks_git_ext_protocol_integration() {
    // Enable Safe Mode
    let mut config = SafeModeConfig::default();
    config.enabled = true;
    let app = create_router_with_config(config);

    // Attempt to use the ext:: protocol which allows command execution
    let payload = ScanRequest {
        input_path: "ext::sh -c touch /tmp/pwned".to_string(),
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

    // Must be Forbidden (403) due to security check, NOT Internal Server Error (500)
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
