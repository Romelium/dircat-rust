#![cfg(feature = "web")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use http_body_util::BodyExt;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
#[cfg(unix)] // Windows does not allow < or > in filenames
async fn test_web_scan_handles_xss_filenames_safely() {
    let temp = tempdir().unwrap();

    // Create a file with an XSS payload in the name
    let malicious_name = "<script>alert(1)</script>.txt";
    fs::write(temp.path().join(malicious_name), "content").unwrap();

    let app = create_router();

    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
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
        request_id: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json_str = String::from_utf8_lossy(&body);

    // The JSON serializer (serde_json) MUST escape the < and > characters
    // to ensure the JSON itself is valid and doesn't break parsers.
    // We expect the raw string to contain the escaped version or the raw chars inside quotes.
    // In JSON: "<" becomes "\u003c" or just "<" inside a string.
    // The critical part is that the response is valid JSON.
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("Response was not valid JSON");

    let found = v.as_array().unwrap().iter().any(|f| {
        f["relative_path"].as_str() == Some(malicious_name)
    });

    assert!(found, "Malicious filename was not correctly preserved in JSON output");
}

#[tokio::test]
#[cfg(unix)]
async fn test_web_scan_handles_control_chars_in_filename() {
    let temp = tempdir().unwrap();

    // Filename with a newline and a bell character
    let malicious_name = "bad\nfile\x07.txt";
    fs::write(temp.path().join(malicious_name), "content").unwrap();

    let app = create_router();

    let payload = ScanRequest {
        input_path: temp.path().to_str().unwrap().to_string(),
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
        request_id: None,
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    // Ensure it parses as valid JSON (serde handles control char escaping automatically)
    let _v: serde_json::Value = serde_json::from_slice(&body).expect("Invalid JSON from control chars");
}
