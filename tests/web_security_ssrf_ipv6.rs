use dircat::security::SafeModeConfig;

#[test]
fn test_validate_input_blocks_ipv6_loopback_variations() {
    let mut config = SafeModeConfig::strict();
    // Allow the domain string, but the IP check should catch the resolution
    config.allowed_domains = Some(vec!["localhost".to_string(), "[::1]".to_string()]);

    // Standard IPv6 Loopback
    let res = config.validate_input("https://[::1]/repo");
    assert!(res.is_err(), "Should block [::1]");

    // IPv6 Loopback Long Form
    let res = config.validate_input("https://[0000:0000:0000:0000:0000:0000:0000:0001]/repo");
    assert!(res.is_err(), "Should block long form IPv6 loopback");
}

#[test]
fn test_validate_input_blocks_ipv4_mapped_ipv6() {
    let mut config = SafeModeConfig::strict();
    config.allowed_domains = Some(vec!["127.0.0.1".to_string()]);

    // IPv4-mapped IPv6 address for localhost
    let res = config.validate_input("https://[::ffff:127.0.0.1]/repo");
    assert!(res.is_err(), "Should block IPv4-mapped IPv6 localhost");
}

#[test]
fn test_validate_input_blocks_octal_ipv4() {
    let config = SafeModeConfig::strict();
    
    // 0177.0.0.1 is octal for 127.0.0.1
    // The URL parser or DNS resolver might normalize this. 
    // If it resolves, it must be blocked. If it fails to parse, that's also secure.
    let res = config.validate_input("https://0177.0.0.1/repo");
    
    if let Ok(Some(ip)) = res {
        panic!("Security Fail: Octal IP {} was allowed!", ip);
    }
    // Accepting Err is fine (blocked or parse error)
}

#[test]
fn test_validate_input_blocks_decimal_ipv4() {
    let config = SafeModeConfig::strict();
    
    // 2130706433 is integer for 127.0.0.1
    let res = config.validate_input("https://2130706433/repo");
    
    if let Ok(Some(ip)) = res {
        panic!("Security Fail: Decimal IP {} was allowed!", ip);
    }
}
