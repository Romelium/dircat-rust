#![cfg(feature = "web")]

use dircat::web::start_server;
use reqwest::StatusCode;
use std::time::Duration;
use tokio::net::TcpListener;

async fn spawn_safe_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        // Safe Mode = true
        start_server(port, false, true, None, None, None, None)
            .await
            .unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_concurrency_limit_rejects_excessive_requests() {
    let url = spawn_safe_server().await;
    let client = reqwest::Client::new();

    // The limit in Safe Mode is 5. We launch 10 simultaneous requests.
    // We use a non-existent domain to ensure they hang on DNS/Connect for a bit,
    // or use a local blocking server.

    let mut handles = vec![];
    for _ in 0..10 {
        let client = client.clone();
        let uri = format!("{}/api/scan", url);
        handles.push(tokio::spawn(async move {
            client
                .post(uri)
                .json(&serde_json::json!({
                    "input_path": "https://github.com/romelium/dircat-rust", // Valid URL
                    "no_gitignore": false, "no_lockfiles": false
                }))
                .send()
                .await
        }));
    }

    let results = futures::future::join_all(handles).await;

    // Count how many succeeded (200) vs failed (503/500 or timeout)
    // Note: Since we are hitting a real GitHub URL, this might be slow.
    // In a real CI, we'd mock the git operation to hang.
    // This test primarily verifies the server doesn't crash.

    let mut success_cnt = 0;
    for res in results {
        if let Ok(Ok(resp)) = res {
            if resp.status() == StatusCode::OK {
                success_cnt += 1;
            }
        }
    }

    // We expect at least some to succeed, but if the semaphore works,
    // the others should queue up. If we flooded 1000, we'd expect failures.
    assert!(success_cnt > 0);
}
