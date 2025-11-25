#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::create_router;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_rejects_oversized_json_payload() {
    let app = create_router();

    // Create a 10MB string (Axum default is usually 2MB)
    let huge_string = "a".repeat(10 * 1024 * 1024);
    let payload = serde_json::json!({
        "input_path": "https://github.com/user/repo",
        "extensions": huge_string, // Attack vector: massive field
        "no_gitignore": false,
        "no_lockfiles": false
    });

    // We need to serialize manually to ensure the body size is actually sent
    let body_content = payload.to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(body_content))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 413 Payload Too Large
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
