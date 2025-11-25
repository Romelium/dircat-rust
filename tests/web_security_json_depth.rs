#![cfg(feature = "web")]

use axum::{body::Body, http::Request};
use dircat::web::create_router;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_rejects_deeply_nested_json() {
    let app = create_router();

    // Create deeply nested JSON: {"a": {"a": {"a": ... }}}
    let mut json = String::from("{\"a\":1}");
    for _ in 0..2000 {
        json = format!("{{\"a\":{}}}", json);
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(json))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 Bad Request (Json parse error) or 422, NOT crash
    assert!(response.status().is_client_error());
}
