#![cfg(all(feature = "web", feature = "git"))]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dircat::web::{create_router, ScanRequest};
use tower::ServiceExt;

#[tokio::test]
async fn test_web_concurrent_requests_to_same_repo_dont_fail() {
    let app = create_router();
    // Use a public repo that is small
    let repo_url = "https://github.com/git-fixtures/basic.git";

    let payload = serde_json::to_string(&ScanRequest {
        input_path: repo_url.to_string(),
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
    })
    .unwrap();

    // Spawn 2 simultaneous requests
    let app1 = app.clone();
    let payload1 = payload.clone();
    let t1 = tokio::spawn(async move {
        app1.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(payload1))
                .unwrap(),
        )
        .await
        .unwrap()
    });

    let app2 = app.clone();
    let payload2 = payload.clone();
    let t2 = tokio::spawn(async move {
        app2.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scan")
                .header("Content-Type", "application/json")
                .body(Body::from(payload2))
                .unwrap(),
        )
        .await
        .unwrap()
    });

    let (res1, res2) = tokio::join!(t1, t2);

    let resp1 = res1.unwrap();
    let resp2 = res2.unwrap();

    // Both should succeed (one waits for the lock, or uses the cache after the first finishes)
    // Neither should 500.
    assert_eq!(resp1.status(), StatusCode::OK);
    assert_eq!(resp2.status(), StatusCode::OK);
}
