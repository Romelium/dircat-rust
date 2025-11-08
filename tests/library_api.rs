// tests/library_api.rs

mod common;

use dircat::config::{self, ConfigBuilder, ResolvedInput};
use dircat::core_types::FileInfo;
use dircat::errors::Error;
use dircat::{discover, process, CancellationToken, Config};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

// --- Test Harness for reducing boilerplate ---

/// A helper struct to manage the environment for a single library API test.
struct TestHarness {
    _temp_dir: TempDir,
    root: PathBuf,
    token: CancellationToken,
}

impl TestHarness {
    /// Creates a new test harness with a temporary directory.
    fn new() -> Self {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();
        Self {
            _temp_dir: temp_dir,
            root,
            token: CancellationToken::new(),
        }
    }

    /// Creates a file with content within the harness's temporary directory.
    fn file(&self, path: &str, content: &[u8]) {
        let full_path = self.root.join(path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(full_path, content).unwrap();
    }

    /// Creates a new ConfigBuilder with the input path set to the harness root.
    fn builder(&self) -> ConfigBuilder {
        ConfigBuilder::new().input_path(self.root.to_str().unwrap())
    }
}

/// Helper to build the config and resolve the input path from a builder.
fn build_and_resolve(builder: ConfigBuilder) -> (Config, ResolvedInput) {
    let config = builder.build().unwrap();
    let resolved = config::resolve_input(&config.input_path, &None, None, &None, None).unwrap();
    (config, resolved)
}

/// Helper to create a basic FileInfo for test inputs.
fn create_test_file_info(root: &Path, relative_path: &str) -> FileInfo {
    FileInfo {
        absolute_path: root.join(relative_path),
        relative_path: relative_path.into(),
        size: 0,
        processed_content: None,
        counts: None,
        is_process_last: false,
        process_last_order: None,
        is_binary: false,
    }
}

// --- Tests ---

#[test]
fn test_discover_returns_sorted_iterator() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("c.txt", b"C");
    harness.file("a.rs", b"A");
    harness.file("b.md", b"B");

    let builder = harness.builder().process_last(vec!["*.md".to_string()]);
    let (config, resolved) = build_and_resolve(builder);

    let discovered_paths: Vec<String> = discover(&config, &resolved, &harness.token)?
        .map(|fi| fi.relative_path.to_string_lossy().to_string())
        .collect();

    assert_eq!(
        discovered_paths,
        vec!["a.rs", "c.txt", "b.md"] // Alphabetical, then --last
    );

    Ok(())
}

#[test]
fn test_discover_iterator_with_complex_filters() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("src/main.rs", b"main");
    harness.file("src/lib.rs", b"lib");
    harness.file("tests/integration.rs", b"test");
    harness.file("docs/guide.md", b"docs");

    let builder = harness
        .builder()
        .extensions(vec!["rs".to_string()]) // Only .rs files
        .exclude_path_regex(vec!["^tests/".to_string()]); // But exclude the tests dir

    let (config, resolved) = build_and_resolve(builder);

    let discovered_paths: Vec<String> = discover(&config, &resolved, &harness.token)?
        .map(|fi| fi.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();

    assert_eq!(discovered_paths, vec!["src/lib.rs", "src/main.rs"]); // Sorted

    Ok(())
}

#[test]
fn test_discover_with_no_matching_files() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("a.log", b"log");

    let builder = harness.builder().extensions(vec!["rs".to_string()]); // Filter for something that doesn't exist
    let (config, resolved) = build_and_resolve(builder);

    let discovered_files: Vec<_> = discover(&config, &resolved, &harness.token)?.collect();
    assert!(discovered_files.is_empty());

    Ok(())
}

#[test]
fn test_process_iterator_reads_and_filters_content() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("a.rs", b"// comment\nfn main() {}");
    harness.file("b.txt", b"Hello");
    harness.file("c.bin", b"binary\0data");

    let files_to_process = vec![
        create_test_file_info(&harness.root, "a.rs"),
        create_test_file_info(&harness.root, "b.txt"),
        create_test_file_info(&harness.root, "c.bin"),
    ];

    let builder = harness.builder().remove_comments(true);
    let (config, _) = build_and_resolve(builder);

    let successful_files: Vec<FileInfo> =
        process(files_to_process.into_iter(), &config, &harness.token)
            .collect::<Result<Vec<_>, Error>>()?;

    assert_eq!(successful_files.len(), 2); // Binary file was filtered out

    let file_a = successful_files
        .iter()
        .find(|fi| fi.relative_path.to_str() == Some("a.rs"))
        .unwrap();
    let file_b = successful_files
        .iter()
        .find(|fi| fi.relative_path.to_str() == Some("b.txt"))
        .unwrap();

    assert_eq!(file_a.processed_content, Some("fn main() {}".to_string()));
    assert_eq!(file_b.processed_content, Some("Hello".to_string()));

    Ok(())
}

#[test]
fn test_process_iterator_includes_binary_when_configured() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("data.bin", b"binary\0data");

    let files_to_process = vec![create_test_file_info(&harness.root, "data.bin")];

    let builder = harness.builder().include_binary(true);
    let (config, _) = build_and_resolve(builder);

    let successful_files: Vec<FileInfo> =
        process(files_to_process.into_iter(), &config, &harness.token)
            .collect::<Result<Vec<_>, Error>>()?;

    assert_eq!(successful_files.len(), 1);
    let binary_file = &successful_files[0];
    assert!(binary_file.is_binary);
    assert_eq!(
        binary_file.processed_content,
        Some(String::from_utf8_lossy(b"binary\0data").to_string())
    );

    Ok(())
}

#[test]
fn test_process_iterator_handles_io_error() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("a.txt", b"Content A");
    harness.file("c.txt", b"Content C");

    let files_to_process = vec![
        create_test_file_info(&harness.root, "a.txt"),
        create_test_file_info(&harness.root, "non_existent.txt"),
        create_test_file_info(&harness.root, "c.txt"),
    ];

    let (config, _) = build_and_resolve(harness.builder());
    let mut processed_iter = process(files_to_process.into_iter(), &config, &harness.token);

    // Item 1: Ok
    assert_eq!(
        processed_iter.next().unwrap()?.relative_path.to_str(),
        Some("a.txt")
    );
    // Item 2: Err
    assert!(matches!(
        processed_iter.next().unwrap(),
        Err(Error::Io { .. })
    ));
    // Item 3: Ok
    assert_eq!(
        processed_iter.next().unwrap()?.relative_path.to_str(),
        Some("c.txt")
    );
    // End of iterator
    assert!(processed_iter.next().is_none());

    Ok(())
}

#[test]
fn test_process_iterator_handles_cancellation() {
    let harness = TestHarness::new();
    harness.file("a.txt", b"A");
    harness.file("b.txt", b"B");

    let files_to_process = vec![
        create_test_file_info(&harness.root, "a.txt"),
        create_test_file_info(&harness.root, "b.txt"),
    ];

    let (config, _) = build_and_resolve(harness.builder());
    harness.token.cancel(); // Cancel *before* processing

    let mut processed_iter = process(files_to_process.into_iter(), &config, &harness.token);

    // The first item pulled from the iterator should immediately be an Interrupted error.
    let result = processed_iter.next().unwrap();
    assert!(matches!(result, Err(Error::Interrupted)));

    // The iterator might yield another error or None, but it shouldn't yield Ok data.
    // Depending on implementation, it might stop after the first error.
}

#[test]
fn test_process_with_empty_iterator() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    let (config, _) = build_and_resolve(harness.builder());
    let files_to_process: Vec<FileInfo> = vec![];

    let processed_files: Vec<_> = process(files_to_process.into_iter(), &config, &harness.token)
        .collect::<Result<Vec<_>, Error>>()?;

    assert!(processed_files.is_empty());
    Ok(())
}

#[test]
fn test_discover_and_process_chaining() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    harness.file("src/main.rs", b"// main\nfn main(){}");
    harness.file("src/data.bin", b"\0\x01\x02");
    harness.file("README.md", b"# Project");
    harness.file(".gitignore", b"*.log");
    harness.file("debug.log", b"Log data");

    let builder = harness
        .builder()
        .remove_comments(true)
        .process_last(vec!["README.md".to_string()]);
    let (config, resolved) = build_and_resolve(builder);

    // --- The Streaming Pipeline ---
    let discovered_iter = discover(&config, &resolved, &harness.token)?;
    let processed_iter = process(discovered_iter, &config, &harness.token);
    let final_files: Vec<FileInfo> = processed_iter.collect::<Result<Vec<_>, Error>>()?;
    // -----------------------------

    assert_eq!(final_files.len(), 2);

    assert_eq!(
        final_files[0]
            .relative_path
            .to_string_lossy()
            .replace('\\', "/"),
        "src/main.rs"
    );
    assert_eq!(
        final_files[0].processed_content,
        Some("fn main(){}".to_string())
    );

    assert_eq!(
        final_files[1]
            .relative_path
            .to_string_lossy()
            .replace('\\', "/"),
        "README.md"
    );
    assert_eq!(
        final_files[1].processed_content,
        Some("# Project".to_string())
    );
    assert!(final_files[1].is_process_last);

    Ok(())
}

#[cfg(feature = "git")]
mod git_feature_tests {
    use super::*;
    use dircat::output::OutputFormatter;

    // A custom formatter for testing purposes that relies on serde_json from the 'git' feature
    struct JsonFormatter;
    impl OutputFormatter for JsonFormatter {
        fn format(
            &self,
            files: &[FileInfo],
            _config: &Config,
            writer: &mut dyn std::io::Write,
        ) -> anyhow::Result<()> {
            // serde_json is available because the tests run with the 'git' feature enabled
            let paths: Vec<_> = files
                .iter()
                .map(|f| f.relative_path.to_string_lossy())
                .collect();
            let json = ::serde_json::json!({ "files": paths });
            write!(writer, "{}", json)?;
            Ok(())
        }
        fn format_dry_run(
            &self,
            files: &[FileInfo],
            _config: &Config,
            writer: &mut dyn std::io::Write,
        ) -> anyhow::Result<()> {
            let paths: Vec<_> = files
                .iter()
                .map(|f| f.relative_path.to_string_lossy())
                .collect();
            let json = ::serde_json::json!({ "dry_run_files": paths });
            write!(writer, "{}", json)?;
            Ok(())
        }
    }

    #[test]
    fn test_dircat_result_format_with_custom_formatter() -> anyhow::Result<()> {
        let harness = TestHarness::new();
        harness.file("a.rs", b"A");
        harness.file("b.txt", b"B");

        let builder = harness.builder();
        let (config, _resolved) = build_and_resolve(builder);

        // Use the full pipeline via dircat::execute
        let result = dircat::execute(&config, &harness.token, None)?;

        // Format using the custom JSON formatter
        let mut buffer = Vec::new();
        result.format_with(&JsonFormatter, &config, &mut buffer)?;

        let output_str = String::from_utf8(buffer)?;
        // The order is deterministic because `execute` sorts the files.
        let expected_json = r#"{"files":["a.rs","b.txt"]}"#;
        assert_eq!(output_str, expected_json);

        Ok(())
    }
}
