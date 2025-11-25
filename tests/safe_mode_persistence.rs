#![cfg(feature = "web")]

use reqwest::StatusCode;
use std::time::Duration;
use tempfile::tempdir;
use tokio::net::TcpListener;

async fn spawn_safe_server(cache_dir: &std::path::Path) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Set the env var for the server process (simulated here by setting it in the test process)
    // Note: Since tests run in the same process, we must be careful with env vars.
    // We set it before spawning and rely on the fact that `start_server` reads it.
    std::env::set_var("DIRCAT_SAFE_CACHE_DIR", cache_dir);

    tokio::spawn(async move {
        // Safe Mode = true
        dircat::web::start_server(port, false, true, None, None, None, None)
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_web_safe_mode_persists_cache() {
    let cache_temp = tempdir().unwrap();
    let url = spawn_safe_server(cache_temp.path()).await;
    let client = reqwest::Client::new();

    // 1. Trigger a scan of a public repo (using a small one to be polite/fast)
    // We use the basic fixtures repo.
    let repo_url = "https://github.com/git-fixtures/basic.git";

    let resp1 = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": repo_url,
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);

    // 2. Verify cache directory is populated
    let entries: Vec<_> = std::fs::read_dir(cache_temp.path()).unwrap().collect();
    assert!(
        !entries.is_empty(),
        "Cache directory should not be empty after scan"
    );

    // 3. Trigger scan again. It should succeed (and be faster/use cache).
    let resp2 = client
        .post(format!("{}/api/scan", url))
        .json(&serde_json::json!({
            "input_path": repo_url,
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);

    std::env::remove_var("DIRCAT_SAFE_CACHE_DIR");
}

#[tokio::test]
async fn test_web_safe_mode_prunes_cache_on_request() {
    let cache_temp = tempdir().unwrap();
    let cache_path = cache_temp.path().to_path_buf();

    // Create dummy data (1KB)
    let old_repo = cache_path.join("old_repo");
    std::fs::create_dir(&old_repo).unwrap();
    std::fs::write(old_repo.join("data"), vec![0u8; 1024]).unwrap(); // 1KB

    // Configure Safe Mode with tiny limit (10 bytes -> 50 bytes cache limit)
    // and explicit cache dir.
    let mut config = dircat::security::SafeModeConfig::strict();
    config.max_repo_size = 10;
    config.cache_dir = Some(cache_path);
    // Disable domain check to allow localhost (for fail-fast URL)
    config.allowed_domains = None;

    let app = dircat::web::create_router_with_config(config);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = reqwest::Client::new();
    // Use a URL that fails fast (Connection Refused on localhost)
    // but passes HTTPS check.
    let fail_fast_url = "https://localhost:54321/repo.git";

    let _ = client
        .post(format!("http://127.0.0.1:{}/api/scan", port))
        .json(&serde_json::json!({
            "input_path": fail_fast_url,
            "no_gitignore": false, "no_lockfiles": false
        }))
        .send()
        .await;

    assert!(!old_repo.exists(), "Old repo should have been pruned");
}
