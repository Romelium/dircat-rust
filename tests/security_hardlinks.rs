#[cfg(unix)]
use dircat::security::SafeModeConfig;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use tempfile::tempdir;

#[test]
#[cfg(unix)]
fn test_validate_file_access_rejects_hardlinks() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let file_a = root.join("original.txt");
    let file_b = root.join("hardlink.txt");

    // Create original file
    fs::write(&file_a, "secret data").unwrap();

    // Create a hard link
    fs::hard_link(&file_a, &file_b).unwrap();

    let config = SafeModeConfig::strict();

    // Validate the original file (nlink = 2)
    let result_a = config.validate_file_access(&file_a, root, None);
    assert!(
        result_a.is_err(),
        "Should reject original file because it has nlink > 1"
    );
    assert!(result_a
        .unwrap_err()
        .to_string()
        .contains("Hardlinks are not allowed"));

    // Validate the hardlink (nlink = 2)
    let result_b = config.validate_file_access(&file_b, root, None);
    assert!(
        result_b.is_err(),
        "Should reject hardlink because it has nlink > 1"
    );
}

#[test]
#[cfg(unix)]
fn test_validate_file_access_allows_normal_files() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let file = root.join("normal.txt");

    fs::write(&file, "data").unwrap();

    let config = SafeModeConfig::strict();
    let result = config.validate_file_access(&file, root, None);
    assert!(result.is_ok(), "Should allow normal file with nlink = 1");
}
