#![cfg(feature = "git")]

use dircat::git::{download_directory_via_api, parse_github_folder_url};
use std::net::ToSocketAddrs;

#[test]
#[ignore = "requires network access"]
fn test_api_download_with_pinned_ip() {
    // Use a public repo folder
    let url = "https://github.com/git-fixtures/basic/tree/master/go";
    let parsed = parse_github_folder_url(url).expect("Failed to parse URL");

    // Resolve github.com IP manually to simulate what the security module does
    let addrs = "github.com:443"
        .to_socket_addrs()
        .expect("Failed to resolve github.com");
    let ip = addrs
        .into_iter()
        .next()
        .expect("No IP addresses found for github.com")
        .ip();

    println!("Pinning connection to IP: {}", ip);

    // Pass the resolved IP to the download function
    // This verifies that the reqwest client is built correctly with the .resolve() override
    // and that it can successfully connect to the pinned IP.
    let result = download_directory_via_api(&parsed, &None, Some(ip));

    assert!(
        result.is_ok(),
        "Failed to download with pinned IP: {:?}",
        result.err()
    );

    let path = result.unwrap();
    assert!(path.exists());
    assert!(path.join("go/example.go").exists());
}
