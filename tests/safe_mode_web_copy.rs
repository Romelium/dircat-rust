#![cfg(feature = "web")]

use reqwest::StatusCode;
use std::time::Duration;
use tokio::net::TcpListener;

async fn spawn_server(safe: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        dircat::web::start_server(port, false, safe, None, None, None, None)
            .await
            .unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_web_copy_disabled_in_safe_mode() {
    let url = spawn_server(true).await; // Safe mode ON
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/copy", url))
        .json(&serde_json::json!({ "content": "secret" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp
        .text()
        .await
        .unwrap()
        .contains("Clipboard access is disabled"));
}

#[tokio::test]
async fn test_web_copy_enabled_in_unsafe_mode() {
    let url = spawn_server(false).await; // Safe mode OFF
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/copy", url))
        .json(&serde_json::json!({ "content": "secret" }))
        .send()
        .await
        .unwrap();

    // Should be OK (or 500 if no clipboard provider, but NOT 403)
    assert_ne!(resp.status(), StatusCode::FORBIDDEN);
}
