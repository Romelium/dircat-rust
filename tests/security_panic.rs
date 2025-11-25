use dircat::security::SafeModeConfig;

#[test]
fn test_panic_message_redaction_logic() {
    // We simulate the logic used inside the panic hook in src/main.rs
    // logic: msg.replace(env!("CARGO_MANIFEST_DIR"), "<redacted>").replace(std::path::MAIN_SEPARATOR, "/")

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let sensitive_path = format!("{}/src/secret.rs", manifest_dir);
    let panic_msg = format!("Panicked at '{}'", sensitive_path);

    // Apply redaction logic
    let redacted = panic_msg
        .replace(manifest_dir, "<redacted>")
        .replace(std::path::MAIN_SEPARATOR, "/");

    assert!(
        !redacted.contains(manifest_dir),
        "Manifest dir should be redacted"
    );
    assert!(
        redacted.contains("<redacted>/src/secret.rs"),
        "Path should be replaced"
    );
}

#[test]
fn test_safe_mode_sanitize_error() {
    let config = SafeModeConfig::strict();

    // Simulate a panic message or error containing an absolute path
    let error_msg = "Error opening file /home/user/dircat/src/main.rs: Permission denied";
    let sanitized = config.sanitize_error(error_msg.to_string());

    assert_eq!(
        sanitized,
        "Error opening file <path_redacted>: Permission denied"
    );
}
