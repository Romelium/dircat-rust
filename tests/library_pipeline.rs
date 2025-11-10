mod common;

use dircat::config::ConfigBuilder;
use dircat::errors::Error;
use dircat::{execute, run, CancellationToken, MarkdownFormatter};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_run_basic_success() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    let output_file_path = temp_dir.path().join("output.md");
    fs::write(temp_dir.path().join("b.txt"), "Content B")?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .output_file(output_file_path.to_str().unwrap())
        .build()?;

    let token = CancellationToken::new();

    // 2. Execute
    let result = run(&config, &token, None);

    // 3. Assert
    assert!(result.is_ok());

    let output_content = fs::read_to_string(&output_file_path)?;
    // discover sorts files alphabetically
    let expected_content =
        "## File: a.rs\n```rs\nfn a() {}\n```\n\n## File: b.txt\n```txt\nContent B\n```\n";
    assert_eq!(output_content, expected_content);

    Ok(())
}

#[test]
fn test_run_dry_run_success() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    let output_file_path = temp_dir.path().join("output.md");
    fs::write(temp_dir.path().join("b.txt"), "Content B")?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .output_file(output_file_path.to_str().unwrap())
        .dry_run(true)
        .build()?;

    let token = CancellationToken::new();

    // 2. Execute
    let result = run(&config, &token, None);

    // 3. Assert
    assert!(result.is_ok());

    let output_content = fs::read_to_string(&output_file_path)?;
    let expected_content =
        "\n--- Dry Run: Files that would be processed ---\n- a.rs\n- b.txt\n--- End Dry Run ---\n";
    assert_eq!(output_content, expected_content);

    Ok(())
}

#[test]
fn test_execute_returns_empty_vec_when_no_files_found() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    let input_path_str = temp_dir.path().to_str().unwrap();
    let config = ConfigBuilder::new().input_path(input_path_str).build()?;
    let token = CancellationToken::new();
    // 2. Execute
    let result = execute(&config, &token, None)?;

    // 3. Assert
    assert!(result.files.is_empty());

    Ok(())
}

#[test]
fn test_run_returns_no_files_found_error() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    let input_path_str = temp_dir.path().to_str().unwrap();
    let config = ConfigBuilder::new().input_path(input_path_str).build()?;
    let token = CancellationToken::new();

    // 2. Execute
    let result = run(&config, &token, None);

    // 3. Assert
    assert!(matches!(result, Err(Error::NoFilesFound)));

    Ok(())
}

#[test]
fn test_run_with_filters_returns_no_files_found() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .extensions(vec!["txt".to_string()]) // Filter for .txt, but only .rs exists
        .build()?;
    let token = CancellationToken::new();

    // 2. Execute
    let result = run(&config, &token, None);

    // 3. Assert
    assert!(matches!(result, Err(Error::NoFilesFound)));

    Ok(())
}

#[test]
fn test_run_respects_stop_signal() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let input_path_str = temp_dir.path().to_str().unwrap();
    let config = ConfigBuilder::new().input_path(input_path_str).build()?;

    // Simulate an immediate cancellation
    let token = CancellationToken::new();
    token.cancel();

    // 2. Execute
    let result = run(&config, &token, None);

    // 3. Assert
    // The error should be `Interrupted` because the discovery loop checks the signal.
    assert!(matches!(result, Err(Error::Interrupted)));

    Ok(())
}

// --- Rests for execute() ---

#[test]
fn test_execute_normal_run_returns_processed_files() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() { /* comment */ }")?;
    fs::write(temp_dir.path().join("b.txt"), "Content B")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .remove_comments(true) // Enable a processing step
        .build()?;
    let token = CancellationToken::new();

    // 2. Execute
    let result = execute(&config, &token, None)?;

    // 3. Assert
    assert_eq!(result.files.len(), 2);
    // Files are sorted alphabetically by discover()
    let file_a = &result.files[0];
    let file_b = &result.files[1];

    assert_eq!(file_a.relative_path.to_str(), Some("a.rs"));
    assert_eq!(file_a.processed_content, Some("fn a() {  }".to_string())); // Comment removed

    assert_eq!(file_b.relative_path.to_str(), Some("b.txt"));
    assert_eq!(file_b.processed_content, Some("Content B".to_string()));

    Ok(())
}

#[test]
fn test_execute_dry_run_returns_unprocessed_files() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() { /* comment */ }")?;
    fs::write(temp_dir.path().join("b.txt"), "Content B")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .dry_run(true)
        .remove_comments(true) // This should be ignored in dry run
        .build()?;
    let token = CancellationToken::new();

    // 2. Execute
    let result = execute(&config, &token, None)?;

    // 3. Assert
    assert_eq!(result.files.len(), 2);
    let file_a = &result.files[0];
    let file_b = &result.files[1];

    assert_eq!(file_a.relative_path.to_str(), Some("a.rs"));
    assert!(file_a.processed_content.is_none()); // Content not read in dry run

    assert_eq!(file_b.relative_path.to_str(), Some("b.txt"));
    assert!(file_b.processed_content.is_none());

    Ok(())
}

#[test]
fn test_execute_dry_run_filters_binary_files() -> anyhow::Result<()> {
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.txt"), "text file")?;
    fs::write(temp_dir.path().join("b.bin"), b"binary\0data")?;

    fs::write(temp_dir.path().join("a.txt"), "text file")?;
    fs::write(temp_dir.path().join("b.bin"), b"binary\0data")?;

    let input_path_str = temp_dir.path().to_str().unwrap();

    let config = ConfigBuilder::new()
        .input_path(input_path_str)
        .dry_run(true)
        .build()?;
    let token = CancellationToken::new();

    // 2. Execute
    let result = execute(&config, &token, None)?;

    // 3. Assert
    assert_eq!(result.files.len(), 1); // Binary file should be filtered out
    assert_eq!(result.files[0].relative_path.to_str(), Some("a.txt"));

    Ok(())
}

#[test]
fn test_format_to_string() -> anyhow::Result<()> {
    // This test now verifies the new recommended way of getting a string.
    // 1. Setup
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}")?;

    let input_path_str = temp_dir.path().to_str().unwrap();
    let config = ConfigBuilder::new().input_path(input_path_str).build()?;
    let token = CancellationToken::new();

    // 2. Execute to get the result
    let result = execute(&config, &token, None)?;

    // 3. Format to a buffer and then to a string
    let mut buffer = Vec::new();
    let output_config = dircat::OutputConfig::from(&config);
    result.format_with(&MarkdownFormatter, &output_config, &mut buffer)?;
    let output_string = String::from_utf8(buffer)?;

    // 4. Assert
    let expected_content = "## File: a.rs\n```rs\nfn a() {}\n```\n";
    assert_eq!(output_string, expected_content);

    Ok(())
}
