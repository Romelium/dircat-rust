#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::create_router;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_endpoints_reject_wrong_methods() {
    let app = create_router();

    // Try GET on /api/scan
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/scan")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
