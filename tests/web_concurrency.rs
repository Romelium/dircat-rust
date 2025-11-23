#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use futures::StreamExt;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;
use tower::ServiceExt;

// Helper to create a dummy file structure
fn setup_env() -> (tempfile::TempDir, String) {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("file.txt"), "content").unwrap();
    let path = temp.path().to_str().unwrap().to_string();
    (temp, path)
}

// Helper to connect to SSE
async fn connect_sse(
    app: axum::Router,
    request_id: Option<&str>,
) -> impl futures::Stream<Item = Result<axum::body::Bytes, axum::Error>> {
    let uri = if let Some(id) = request_id {
        format!("/api/events?request_id={}", id)
    } else {
        "/api/events".to_string()
    };

    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.into_body().into_data_stream()
}

// Helper to trigger a scan
async fn trigger_scan(app: axum::Router, input_path: String, request_id: Option<String>) {
    let payload = ScanRequest {
        input_path,
        max_size: None,
        extensions: None,
        exclude_extensions: None,
        no_gitignore: false,
        no_lockfiles: false,
        git_branch: None,
        git_depth: None,
        path_regex: None,
        filename_regex: None,
        exclude_path_regex: None,
        ignore_patterns: None,
        request_id,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/api/scan")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_sse_isolation_distinct_clients() {
    let (_temp, input_path) = setup_env();
    let app = create_router();

    // Client A connects with ID "client_a"
    let mut stream_a = connect_sse(app.clone(), Some("client_a")).await;

    // Client B triggers scan with ID "client_b"
    let app_clone = app.clone();
    let input_path_clone = input_path.clone();
    tokio::spawn(async move {
        trigger_scan(app_clone, input_path_clone, Some("client_b".to_string())).await;
    });

    // Client A should NOT receive events
    let next_item = tokio::time::timeout(Duration::from_millis(200), stream_a.next()).await;

    if let Ok(Some(Ok(bytes))) = next_item {
        let s = String::from_utf8(bytes.to_vec()).unwrap();
        // If we get data, it shouldn't be progress messages
        assert!(
            !s.contains("Scanning directory"),
            "Client A received Client B's events: {}",
            s
        );
    }
}

#[tokio::test]
async fn test_sse_concurrent_clients_receive_own_events() {
    let (_temp, input_path) = setup_env();
    let app = create_router();

    let mut stream_a = connect_sse(app.clone(), Some("client_a")).await;
    let mut stream_b = connect_sse(app.clone(), Some("client_b")).await;

    let app_a = app.clone();
    let path_a = input_path.clone();
    tokio::spawn(async move {
        trigger_scan(app_a, path_a, Some("client_a".to_string())).await;
    });

    let app_b = app.clone();
    let path_b = input_path.clone();
    tokio::spawn(async move {
        trigger_scan(app_b, path_b, Some("client_b".to_string())).await;
    });

    // Verify A received something relevant
    let item_a = tokio::time::timeout(Duration::from_secs(2), stream_a.next())
        .await
        .expect("Client A timed out")
        .expect("Stream A ended")
        .expect("Stream A error");
    let s_a = String::from_utf8(item_a.to_vec()).unwrap();
    assert!(
        s_a.contains("progress") || s_a.contains("done"),
        "Client A got unexpected data: {}",
        s_a
    );

    // Verify B received something relevant
    let item_b = tokio::time::timeout(Duration::from_secs(2), stream_b.next())
        .await
        .expect("Client B timed out")
        .expect("Stream B ended")
        .expect("Stream B error");
    let s_b = String::from_utf8(item_b.to_vec()).unwrap();
    assert!(
        s_b.contains("progress") || s_b.contains("done"),
        "Client B got unexpected data: {}",
        s_b
    );
}

#[tokio::test]
async fn test_sse_shared_session_id() {
    let (_temp, input_path) = setup_env();
    let app = create_router();
    let session_id = "shared_session";

    let mut stream_1 = connect_sse(app.clone(), Some(session_id)).await;
    let mut stream_2 = connect_sse(app.clone(), Some(session_id)).await;

    let app_clone = app.clone();
    tokio::spawn(async move {
        trigger_scan(app_clone, input_path, Some(session_id.to_string())).await;
    });

    let item_1 = tokio::time::timeout(Duration::from_secs(1), stream_1.next()).await;
    let item_2 = tokio::time::timeout(Duration::from_secs(1), stream_2.next()).await;

    assert!(item_1.is_ok(), "Stream 1 did not receive event");
    assert!(item_2.is_ok(), "Stream 2 did not receive event");
}

#[tokio::test]
async fn test_sse_default_global_fallback() {
    let (_temp, input_path) = setup_env();
    let app = create_router();

    // Connect without ID (defaults to "global")
    let mut stream = connect_sse(app.clone(), None).await;

    let app_clone = app.clone();
    tokio::spawn(async move {
        // Trigger without ID (defaults to "global")
        trigger_scan(app_clone, input_path, None).await;
    });

    let item = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
    assert!(item.is_ok(), "Default global stream did not receive event");
}
