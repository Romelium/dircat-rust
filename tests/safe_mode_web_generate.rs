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
async fn test_web_generate_blocks_output_file() {
    let url = spawn_server(true).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "input_path": "https://github.com/user/repo",
            "output_file": "/tmp/hacked.txt", // Attempt to write to server disk
            "remove_comments": false, "remove_empty_lines": false, "include_binary": false,
            "line_numbers": false, "filename_only": false, "backticks": false,
            "summary": false, "counts": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(resp
        .text()
        .await
        .unwrap()
        .contains("Server-side file writing is disabled"));
}

#[tokio::test]
async fn test_web_generate_blocks_path_traversal_selection() {
    let url = spawn_server(true).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "input_path": "https://github.com/user/repo",
            "selected_files": ["../../etc/passwd"], // Attempt traversal
            "remove_comments": false, "remove_empty_lines": false, "include_binary": false,
            "line_numbers": false, "filename_only": false, "backticks": false,
            "summary": false, "counts": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp
        .text()
        .await
        .unwrap()
        .contains("Invalid file path in selection"));
}

#[tokio::test]
async fn test_web_generate_blocks_absolute_path_selection() {
    let url = spawn_server(true).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "input_path": "https://github.com/user/repo",
            "selected_files": ["/etc/passwd"], // Absolute path
            "remove_comments": false, "remove_empty_lines": false, "include_binary": false,
            "line_numbers": false, "filename_only": false, "backticks": false,
            "summary": false, "counts": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
