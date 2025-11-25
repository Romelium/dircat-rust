#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::security::SafeModeConfig;
use dircat::web::create_router_with_config;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn test_web_scan_terminates_on_hanging_server() {
    // 1. Setup a "Blackhole" Server
    // Accepts TCP connections but never writes back, causing clients to hang indefinitely.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            if let Ok((_socket, _)) = listener.accept().await {
                // Just hold the socket open without reading/writing until test ends
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    });

    // 2. Configure Dircat with a short timeout
    let mut config = SafeModeConfig::strict();
    config.request_timeout = Duration::from_millis(500); // 0.5s timeout
                                                         // We must allow local IPs to connect to our test server
    config.allowed_domains = Some(vec!["127.0.0.1".to_string()]);
    // We must allow HTTP because our blackhole server isn't doing TLS handshakes
    // (Note: SafeMode usually blocks HTTP, so we bypass validation logic by using a custom router
    // or we rely on the fact that the timeout happens during connection attempt)
    // Actually, SafeMode enforces HTTPS in `validate_input`.
    // To test the *timeout logic* specifically, we disable the protocol check by disabling SafeMode
    // but keeping the timeout config manually, OR we assume the timeout applies to the connection attempt.

    // Let's disable SafeMode validation checks but keep the timeout logic in the handler.
    // The handler uses `state.security.request_timeout`.
    config.enabled = false;
    // Even if enabled is false, the handler uses the timeout value from the config struct.

    let app = create_router_with_config(config);

    // 3. Send Request
    let payload = serde_json::json!({
        "input_path": format!("http://127.0.0.1:{}/repo.git", port),
        "no_gitignore": false, "no_lockfiles": false
    });

    let start = std::time::Instant::now();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let duration = start.elapsed();

    // 4. Assertions
    // Should return 408 Request Timeout
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);

    // Should happen roughly around the timeout duration (allow slight overhead)
    assert!(duration >= Duration::from_millis(500));
    assert!(
        duration < Duration::from_secs(2),
        "Timeout mechanism failed, took too long"
    );
}
