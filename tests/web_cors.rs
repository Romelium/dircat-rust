#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use dircat::security::SafeModeConfig;
use dircat::web::{create_router, create_router_with_config};
use tower::util::ServiceExt;

#[tokio::test]
async fn test_cors_standard_mode_is_permissive() {
    // Standard mode (Safe Mode = false)
    let app = create_router();

    // Simulate a preflight OPTIONS request from a development server (e.g., Vite/React)
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/scan")
                .header(header::ORIGIN, "http://localhost:5173")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization") // Random header
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // In permissive mode, it should allow everything
    let allow_origin = response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert_eq!(allow_origin.unwrap(), "*");

    // It should reflect or allow the requested headers
    let allow_headers = response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS);
    // Permissive usually echoes, or allows all. Just checking it exists and isn't empty is usually enough for permissive check.
    assert!(allow_headers.is_some());
}

#[tokio::test]
async fn test_cors_safe_mode_allows_valid_api_usage() {
    let mut config = SafeModeConfig::default();
    config.enabled = true; // Enable Safe Mode
    let app = create_router_with_config(config);

    // Valid request: POST with Content-Type
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/generate")
                .header(header::ORIGIN, "https://some-tool.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Safe mode still allows Any origin (*) for tool interoperability
    let allow_origin = response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    assert_eq!(allow_origin.unwrap(), "*");

    // Should allow POST
    let allow_methods = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(allow_methods.contains("POST"));

    // Should allow Content-Type
    let allow_headers = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(allow_headers.to_lowercase().contains("content-type"));
}

#[tokio::test]
async fn test_cors_safe_mode_restricts_methods() {
    let mut config = SafeModeConfig::default();
    config.enabled = true;
    let app = create_router_with_config(config);

    // Invalid request: DELETE method (not used by dircat API)
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/scan")
                .header(header::ORIGIN, "https://malicious.com")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // The Access-Control-Allow-Methods header should NOT contain DELETE.
    // Depending on implementation, the header might be missing entirely if the requested method isn't allowed,
    // or it might return the list of allowed methods (GET, POST).
    if let Some(methods_val) = response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS) {
        let methods = methods_val.to_str().unwrap();
        assert!(
            !methods.contains("DELETE"),
            "Safe Mode CORS should not allow DELETE. Allowed: {}",
            methods
        );
    }
}
