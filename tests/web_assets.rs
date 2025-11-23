#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::create_router;
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_index_html_contains_robust_copy_logic() {
    let app = create_router();

    // Request the root path (index.html)
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8_lossy(&body);

    // 1. Verify Modern API usage
    assert!(
        html.contains("navigator.clipboard.writeText"),
        "index.html missing modern clipboard API logic"
    );

    // 2. Verify Legacy Fallback (execCommand)
    assert!(
        html.contains("document.execCommand('copy')"),
        "index.html missing legacy execCommand fallback"
    );

    // 3. Verify Manual Selection Fallback
    assert!(
        html.contains("range.selectNodeContents"),
        "index.html missing manual selection fallback logic"
    );

    assert!(
        html.contains("Copy failed. Text selected"),
        "index.html missing user alert for manual selection"
    );
}
