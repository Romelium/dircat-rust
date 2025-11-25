use dircat::config::ConfigBuilder;
use dircat::security::SafeModeConfig;
use dircat::{execute, CancellationToken};
use std::fs;
use tempfile::tempdir;

#[test]
#[cfg(unix)]
fn test_core_symlink_jailbreak_prevention() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path();

    // 1. Setup the "Jail" and the "Secret" outside it
    let secret_file = root.join("secret_password.txt");
    fs::write(&secret_file, "super_secret_value").unwrap();

    let jail_dir = root.join("repo");
    fs::create_dir(&jail_dir).unwrap();

    // 2. Create a symlink inside the jail pointing out
    let link_path = jail_dir.join("config.txt");
    symlink(&secret_file, &link_path).unwrap();

    // 3. Configure Safe Mode
    let mut config = ConfigBuilder::new()
        .input_path(jail_dir.to_str().unwrap())
        .build()
        .unwrap();

    // Enable discovery protection (walker shouldn't follow links)
    config.discovery.safe_mode = true;

    // Enable processing protection (double check LFI)
    let mut sec = SafeModeConfig::strict();
    sec.allow_symlinks = false;
    config.processing.security = Some(sec);

    // 4. Execute
    let token = CancellationToken::new();
    let result = execute(&config, &token, None).unwrap();

    // 5. Assert
    // The file should effectively be invisible or empty.
    // It definitely must NOT contain the secret.
    let leaked = result
        .files
        .iter()
        .any(|f| f.processed_content.as_deref() == Some("super_secret_value"));

    assert!(
        !leaked,
        "SECURITY FAIL: Symlink followed outside of jail root!"
    );
}

#[test]
fn test_core_max_file_size_limit() {
    let temp = tempdir().unwrap();
    let jail_dir = temp.path();

    // Create a 2MB file
    let large_content = "0".repeat(2 * 1024 * 1024);
    fs::write(jail_dir.join("large.db"), &large_content).unwrap();

    // Create a small file
    fs::write(jail_dir.join("small.txt"), "ok").unwrap();

    let mut config = ConfigBuilder::new()
        .input_path(jail_dir.to_str().unwrap())
        .build()
        .unwrap();

    // Set limit to 1MB
    let mut sec = SafeModeConfig::strict();
    sec.max_file_size = 1 * 1024 * 1024;
    config.processing.security = Some(sec);

    let token = CancellationToken::new();
    let result = execute(&config, &token, None).unwrap();

    // Assert small file is present
    assert!(result
        .files
        .iter()
        .any(|f| f.relative_path.to_str() == Some("small.txt")));

    // Assert large file is ABSENT (filtered out)
    let large_present = result
        .files
        .iter()
        .any(|f| f.relative_path.to_str() == Some("large.db"));
    assert!(
        !large_present,
        "SECURITY FAIL: File larger than max_file_size was processed!"
    );
}

#[test]
#[cfg(unix)]
fn test_core_symlink_loop_does_not_hang() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let root = temp.path();

    // Create a recursive symlink loop: root/loop -> root
    let link_path = root.join("loop");
    symlink(root, &link_path).unwrap();

    let mut config = ConfigBuilder::new()
        .input_path(root.to_str().unwrap())
        .build()
        .unwrap();

    // Enable Safe Mode (disables follow_links)
    config.discovery.safe_mode = true;

    let token = CancellationToken::new();

    // This should finish quickly and not hang/crash
    let result = execute(&config, &token, None);

    assert!(result.is_ok());
}
