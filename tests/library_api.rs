// tests/library_api.rs

mod common;

use dircat::config::{self, ConfigBuilder, ResolvedInput};
use dircat::core_types::FileInfo;
use dircat::errors::Error;
use dircat::{discover, process_files, CancellationToken, Config};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

use dircat::core_types::FileContent;
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
    let mut fi = FileInfo::default();
    fi.absolute_path = root.join(relative_path);
    fi.relative_path = relative_path.into();
    fi
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

    let discovered_paths: Vec<String> = discover(&config.discovery, &resolved, &harness.token)?
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

    let discovered_paths: Vec<String> = discover(&config.discovery, &resolved, &harness.token)?
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

    let discovered_files: Vec<_> =
        discover(&config.discovery, &resolved, &harness.token)?.collect();
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

    let successful_files: Vec<FileInfo> = process_files(
        files_to_process.into_iter(),
        &config.processing,
        &harness.token,
    )
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

    let successful_files: Vec<FileInfo> = process_files(
        files_to_process.into_iter(),
        &config.processing,
        &harness.token,
    )
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
    let processed_iter = process_files(
        files_to_process.into_iter(),
        &config.processing,
        &harness.token,
    );

    // With parallel processing, order is not guaranteed. Collect all results and inspect.
    let results: Vec<_> = processed_iter.collect();
    assert_eq!(results.len(), 3);

    let mut ok_count = 0;
    let mut err_count = 0;
    let mut ok_paths = std::collections::HashSet::new();

    for result in results {
        match result {
            Ok(fi) => {
                ok_count += 1;
                ok_paths.insert(fi.relative_path.to_string_lossy().to_string());
            }
            Err(Error::Io { .. }) => {
                err_count += 1;
            }
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }
    }

    assert_eq!(ok_count, 2);
    assert_eq!(err_count, 1);
    assert!(ok_paths.contains("a.txt"));
    assert!(ok_paths.contains("c.txt"));

    Ok(())
}

#[test]
fn test_process_content_decoupled_from_io() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    // No files are actually created on the filesystem.

    let files_content = vec![
        {
            let mut fc = FileContent::default();
            fc.relative_path = "a.rs".into();
            fc.content = b"// comment\nfn main() {}".to_vec();
            fc
        },
        {
            let mut fc = FileContent::default();
            fc.relative_path = "b.txt".into();
            fc.content = b"Hello".to_vec();
            fc
        },
        {
            let mut fc = FileContent::default();
            fc.relative_path = "c.bin".into();
            fc.content = b"binary\0data".to_vec();
            fc
        },
    ];

    let builder = harness.builder().remove_comments(true);
    let (config, _) = build_and_resolve(builder);
    let opts = dircat::processing::ProcessingOptions::from(&config);

    let successful_files: Vec<FileInfo> =
        dircat::process_content(files_content.into_iter(), opts, &harness.token)
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

    let results: Vec<_> = process_files(
        files_to_process.into_iter(),
        &config.processing,
        &harness.token,
    )
    .collect();

    // With parallel execution, we might get one or more Interrupted errors.
    // We should check that at least one is present.
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| matches!(r, Err(Error::Interrupted))));
}

#[test]
fn test_process_with_empty_iterator() -> anyhow::Result<()> {
    let harness = TestHarness::new();
    let (config, _) = build_and_resolve(harness.builder());
    let files_to_process: Vec<FileInfo> = vec![];

    let processed_files: Vec<_> = process_files(
        files_to_process.into_iter(),
        &config.processing,
        &harness.token,
    )
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
    let discovered_iter = discover(&config.discovery, &resolved, &harness.token)?;
    let processed_iter = process_files(discovered_iter, &config.processing, &harness.token);
    let mut final_files: Vec<FileInfo> = processed_iter.collect::<Result<Vec<_>, Error>>()?;
    // -----------------------------

    // Re-sort the files after parallel processing, which does not preserve order.
    final_files.sort_by_key(|fi| {
        (
            fi.is_process_last,
            fi.process_last_order,
            fi.relative_path.clone(),
        )
    });

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
            _opts: &dircat::OutputConfig,
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
            _opts: &dircat::OutputConfig,
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
        let output_config = dircat::OutputConfig::from(&config);
        result.format_with(&JsonFormatter, &output_config, &mut buffer)?;

        let output_str = String::from_utf8(buffer)?;
        // The order is deterministic because `execute` sorts the files.
        let expected_json = r#"{"files":["a.rs","b.txt"]}"#;
        assert_eq!(output_str, expected_json);

        Ok(())
    }
}
