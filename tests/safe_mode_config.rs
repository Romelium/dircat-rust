// tests/safe_mode_config.rs

use dircat::security::SafeModeConfig;

#[test]
fn test_config_strict_defaults() {
    let config = SafeModeConfig::strict();
    assert!(config.enabled);
    assert!(!config.allow_local_paths);
    assert!(!config.allow_clipboard);
    assert!(!config.allow_symlinks);
    assert_eq!(
        config.allowed_domains,
        Some(vec!["github.com".to_string(), "gitlab.com".to_string()])
    );
}

#[test]
fn test_validate_input_protocols() {
    let config = SafeModeConfig::strict();

    // Allowed
    assert!(config
        .validate_input("https://github.com/user/repo")
        .is_ok());

    // Blocked
    assert!(
        config
            .validate_input("http://github.com/user/repo")
            .is_err(),
        "HTTP should be blocked"
    );
    assert!(
        config
            .validate_input("ssh://git@github.com/user/repo")
            .is_err(),
        "SSH should be blocked"
    );
    assert!(
        config
            .validate_input("git@github.com:user/repo.git")
            .is_err(),
        "SCP-like SSH should be blocked"
    );
    assert!(
        config.validate_input("file:///etc/passwd").is_err(),
        "File protocol should be blocked"
    );
    assert!(
        config.validate_input("ftp://github.com/repo").is_err(),
        "FTP should be blocked"
    );
}

#[test]
fn test_validate_input_local_paths() {
    let config = SafeModeConfig::strict();

    assert!(config.validate_input("/etc/passwd").is_err());
    assert!(config.validate_input("./src").is_err());
    assert!(config.validate_input("C:\\Windows").is_err());
    assert!(config.validate_input("~/.ssh").is_err());
}

#[test]
fn test_validate_input_domain_allowlist() {
    let mut config = SafeModeConfig::strict();
    config.allowed_domains = Some(vec!["example.com".to_string()]);

    // Exact match
    // Note: We use www.example.com for subdomain test because it resolves,
    // preventing DNS check failure in validate_input.
    assert!(config.validate_input("https://example.com/repo").is_ok());

    // Subdomain match
    assert!(config
        .validate_input("https://www.example.com/repo")
        .is_ok());

    // Mismatch
    assert!(config.validate_input("https://github.com/repo").is_err());
    assert!(config
        .validate_input("https://notexample.com/repo")
        .is_err());

    // Phishing attempt (suffix match but not subdomain)
    assert!(config.validate_input("https://myexample.com/repo").is_err());
}

#[test]
fn test_validate_input_wildcard_domains() {
    let mut config = SafeModeConfig::strict();
    config.allowed_domains = None; // Allow all

    assert!(config
        .validate_input("https://random-site.com/repo.git")
        .is_ok());
    // Protocol restriction still applies
    assert!(config
        .validate_input("http://random-site.com/repo.git")
        .is_err());
}

#[test]
fn test_validate_input_rejects_homographs() {
    let mut config = SafeModeConfig::strict();
    config.allowed_domains = Some(vec!["github.com".to_string()]);

    // Cyrillic 'a' in github.com (xn--githu-r4a.com)
    let fake_github = "https://gith\u{0430}b.com/repo";
    assert!(config.validate_input(fake_github).is_err());
}

#[test]
fn test_validate_input_rejects_ext_transport() {
    let config = SafeModeConfig::strict();
    // ext:: transport allows command execution
    assert!(config.validate_input("ext::sh -c echo pwned").is_err());
}

#[test]
fn test_validate_input_explicitly_blocks_git_ext_protocol() {
    let config = SafeModeConfig::strict();
    // This is a known RCE vector in some git contexts
    let malicious_url = "ext::sh -c touch /tmp/pwned";

    let res = config.validate_input(malicious_url);
    assert!(res.is_err());

    let err = res.unwrap_err().to_string();
    // It might fail as "Invalid URL" (due to spaces) or "Only 'https'..." or "Local paths..."
    // All are valid blocks.
    assert!(
        err.contains("Safe Mode"),
        "Should be blocked by Safe Mode. Got: {}",
        err
    );
}
