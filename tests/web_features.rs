#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::create_router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_features_endpoint_returns_correct_flags() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/features")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Verify the JSON structure matches the compile-time configuration of the test runner
    let expected_git = cfg!(feature = "git");
    let expected_clipboard = cfg!(feature = "clipboard");

    assert_eq!(json["git"], expected_git, "Git feature flag mismatch");
    assert_eq!(
        json["clipboard"], expected_clipboard,
        "Clipboard feature flag mismatch"
    );
}

#[tokio::test]
async fn test_web_features_endpoint_headers() {
    let app = create_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/features")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn test_web_features_endpoint_schema() {
    let app = create_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/features")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json.get("git").is_some() && json["git"].is_boolean());
    assert!(json.get("clipboard").is_some() && json["clipboard"].is_boolean());
}
