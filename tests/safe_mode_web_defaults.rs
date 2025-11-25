#![cfg(feature = "web")]

use reqwest::StatusCode;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;
use tokio::net::TcpListener;

async fn spawn_unsafe_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        // safe = false
        dircat::web::start_server(port, false, false, None, None, None, None)
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_web_unsafe_mode_allows_local_paths() {
    let url = spawn_unsafe_server().await;
    let client = reqwest::Client::new();
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("local.txt");
    fs::write(&file_path, "local content").unwrap();

    // Request a local path
    let resp = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": temp.path().to_str().unwrap(),
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    // Should be OK (200), NOT Forbidden (403)
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.text().await.unwrap();
    assert!(body.contains("local.txt"));
}
