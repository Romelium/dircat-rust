#![cfg(feature = "web")]

use reqwest::StatusCode;
use std::time::Duration;
use tokio::net::TcpListener;

// Shared helper for spawning server
async fn spawn_server(safe: bool, domains: Option<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        dircat::web::start_server(port, false, safe, domains, None, None, None)
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_web_scan_blocks_local_path() {
    let url = spawn_server(true, None).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": "/etc/passwd",
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp
        .text()
        .await
        .unwrap()
        .contains("Local file system paths are disabled"));
}

#[tokio::test]
async fn test_web_scan_blocks_http() {
    let url = spawn_server(true, None).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": "http://github.com/user/repo",
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp
        .text()
        .await
        .unwrap()
        .contains("Only 'https' protocol is allowed"));
}

#[tokio::test]
async fn test_web_scan_blocks_unauthorized_domain() {
    let url = spawn_server(true, Some("mycorp.com".to_string())).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": "https://github.com/user/repo", // Not in allowlist
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp.text().await.unwrap().contains("not in the allowlist"));
}

#[tokio::test]
async fn test_web_scan_allows_authorized_domain() {
    // Note: This will fail later in the pipeline (git clone) because the repo doesn't exist,
    // but we are testing that it passes the *security validation* layer (i.e., not 403 Forbidden).
    let url = spawn_server(true, Some("github.com".to_string())).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": "https://github.com/romelium/dircat-rust",
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    // It might be 200 (if it actually scans) or 500/400 (if git fails),
    // but it MUST NOT be 403 Forbidden.
    assert_ne!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_web_scan_blocks_url_encoded_path_traversal() {
    let url = spawn_server(true, None).await;
    let client = reqwest::Client::new();

    // URL encoded "../" is "%2e%2e%2f"
    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": "file://%2e%2e%2fetc%2fpasswd",
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
