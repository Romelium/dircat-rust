use dircat::security::SafeModeConfig;

#[test]
fn test_validate_input_blocks_localhost_ips() {
    let mut config = SafeModeConfig::strict();
    // Even if we allow "localhost" in the domain list (which is dangerous),
    // the IP check should catch it if it resolves to 127.0.0.1.
    config.allowed_domains = Some(vec!["localhost".to_string(), "127.0.0.1".to_string()]);

    // 1. Direct IP
    let res = config.validate_input("https://127.0.0.1/repo");
    assert!(res.is_err(), "Should block 127.0.0.1");
    assert!(res.unwrap_err().to_string().contains("private/local IP"));

    // 2. Localhost (resolves to 127.0.0.1)
    let res = config.validate_input("https://localhost/repo");
    assert!(res.is_err(), "Should block localhost resolution");
}

#[test]
fn test_validate_input_blocks_private_ranges() {
    let mut config = SafeModeConfig::strict();
    config.allowed_domains = Some(vec!["192.168.1.1".to_string(), "10.0.0.5".to_string()]);

    assert!(config.validate_input("https://192.168.1.1/repo").is_err());
    assert!(config.validate_input("https://10.0.0.5/repo").is_err());
}

#[test]
fn test_validate_input_returns_ip_for_pinning() {
    let mut config = SafeModeConfig::strict();
    // Allow google.com for this test (requires internet)
    config.allowed_domains = Some(vec!["google.com".to_string()]);

    if std::net::ToSocketAddrs::to_socket_addrs("google.com:443").is_ok() {
        let res = config.validate_input("https://google.com/repo");
        assert!(res.is_ok());

        // The fix modifies validate_input to return Option<IpAddr>
        // We verify it returns Some(IP) so the caller can pin it.
        let ip_opt = res.unwrap();
        assert!(ip_opt.is_some(), "Should return resolved IP for pinning");
    }
}

#[test]
fn test_validate_input_blocks_ipv6_loopback() {
    let mut config = SafeModeConfig::strict();
    // Even if allowed in domain list, IP check must catch it
    config.allowed_domains = Some(vec!["[::1]".to_string()]);

    let res = config.validate_input("https://[::1]/repo");
    assert!(res.is_err(), "Should block IPv6 loopback [::1]");
}

#[test]
fn test_validate_input_blocks_decimal_ip_encoding() {
    let config = SafeModeConfig::strict();

    // 2130706433 is the decimal representation of 127.0.0.1
    // If the URL parser or DNS resolver normalizes this, the security check must catch the resulting IP.
    let res = config.validate_input("https://2130706433/repo");

    // It should either fail to parse/resolve (Err) OR resolve to 127.0.0.1 and be blocked (Err).
    assert!(
        res.is_err(),
        "Decimal IP encoding for localhost should be blocked or fail resolution"
    );
}
