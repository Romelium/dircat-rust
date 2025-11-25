#![cfg(feature = "web")]

use dircat::security::SafeModeConfig;
use dircat::web::ScanRequest;

// We need to expose the validation function or test via the handler.
// Since validate_scan_request_complexity is private, we test via the router/handler logic
// or assume we can access it if we make it pub(crate) and use a child module.
// Here we simulate the logic directly or use the integration approach.

#[tokio::test]
async fn test_scan_rejects_excessive_wildcards() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt; // for oneshot // for collect

    // Enable Safe Mode
    let mut config = SafeModeConfig::default();
    config.enabled = true;
    let app = dircat::web::create_router_with_config(config);

    // Pattern with 5 wildcards (Limit is 4)
    let dangerous_pattern = "a*b*c*d*e*f";

    let payload = ScanRequest {
        input_path: "https://github.com/user/repo".to_string(),
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
        ignore_patterns: Some(dangerous_pattern.to_string()),
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
    let msg = String::from_utf8_lossy(&body);
    assert!(msg.contains("too many wildcards"));
}

#[tokio::test]
async fn test_scan_allows_reasonable_wildcards() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let mut config = SafeModeConfig::default();
    config.enabled = true;
    let app = dircat::web::create_router_with_config(config);

    // Pattern with 2 wildcards (Safe)
    let safe_pattern = "*.log";

    let payload = ScanRequest {
        input_path: "https://github.com/user/repo".to_string(), // This will fail later at git clone, which is fine
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
        ignore_patterns: Some(safe_pattern.to_string()),
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

    // Should NOT be 400 Bad Request (might be 500 or 403 depending on git/network, but validation passed)
    assert_ne!(response.status(), StatusCode::BAD_REQUEST);
}
