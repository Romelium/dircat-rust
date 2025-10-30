// tests/git_input.rs

mod common;

use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

/// Helper function to create a local git repository in a temporary directory for testing.
///
/// This sets up a repo with a structure like:
/// - .gitignore (ignores *.log)
/// - README.md
/// - src/main.rs
/// - data/app.log (committed, but should be ignored by dircat's walker)
/// - uncommitted.txt (created locally, but NOT committed)
fn setup_local_git_repo() -> Result<TempDir, Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let repo_path = temp_dir.path();

    // 1. Initialize the repository
    let repo = git2::Repository::init(repo_path)?;

    // 2. Create files and directories
    fs::write(repo_path.join("README.md"), "# Test Repo")?;
    fs::create_dir_all(repo_path.join("src"))?;
    fs::write(repo_path.join("src/main.rs"), "fn main() {}")?;
    fs::write(repo_path.join(".gitignore"), "*.log")?;
    fs::create_dir_all(repo_path.join("data"))?;
    fs::write(repo_path.join("data/app.log"), "some log data")?;
    // This file is created but will NOT be committed.
    fs::write(repo_path.join("uncommitted.txt"), "do not include")?;

    // 3. Stage only the files intended for the commit
    let mut index = repo.index()?;
    index.add_path(std::path::Path::new("README.md"))?;
    index.add_path(std::path::Path::new("src/main.rs"))?;
    index.add_path(std::path::Path::new(".gitignore"))?;
    // We explicitly add the log file to the commit. The test will verify that
    // dircat's walker respects the .gitignore in the *cloned* repo.
    index.add_path(std::path::Path::new("data/app.log"))?;
    index.write()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    // 4. Create a commit
    let signature = git2::Signature::now("Test User", "test@example.com")?;
    repo.commit(
        Some("HEAD"),     // Point HEAD to our new commit
        &signature,       // Author
        &signature,       // Committer
        "Initial commit", // Commit message
        &tree,            // Tree from the index
        &[],              // No parents for the initial commit
    )?;

    Ok(temp_dir)
}

/// An RAII guard to clean up a directory path when it goes out of scope.
struct DirCleanupGuard(PathBuf);

impl Drop for DirCleanupGuard {
    fn drop(&mut self) {
        if self.0.exists() {
            // Don't panic on failure, just print a warning.
            if let Err(e) = fs::remove_dir_all(&self.0) {
                eprintln!(
                    "Warning: Failed to clean up test cache directory '{}': {}",
                    self.0.display(),
                    e
                );
            }
        }
    }
}

/// Tests cloning and processing a local file-based git repository.
/// This test is fast and runs offline.
#[test]
fn test_git_local_repository_input() -> Result<(), Box<dyn std::error::Error>> {
    let source_repo_dir = setup_local_git_repo()?;
    let repo_path_str = source_repo_dir.path().to_str().unwrap();

    // Correctly format the path as a file URL for git2.
    // On Windows, this becomes `file:///C:/...`
    // On Unix, this becomes `file:///path/...`
    #[cfg(windows)]
    let repo_url = format!("file:///{}", repo_path_str.replace('\\', "/"));
    #[cfg(not(windows))]
    let repo_url = format!("file://{}", repo_path_str);

    // This test uses the real cache, so we set up a guard to clean it up.
    // We need to create a unique cache dir for this test run to avoid conflicts.
    let temp_cache = tempdir()?;
    let _cache_guard = DirCleanupGuard(temp_cache.path().to_path_buf());
    let original_xdg_cache_home = env::var("XDG_CACHE_HOME");
    env::set_var("XDG_CACHE_HOME", temp_cache.path());

    dircat_cmd()
        .arg(&repo_url) // Use the file:// URL
        .assert()
        .success()
        // Check that committed files are included
        .stdout(predicate::str::contains("## File: README.md"))
        .stdout(predicate::str::contains("# Test Repo"))
        .stdout(predicate::str::contains("## File: src/main.rs"))
        .stdout(predicate::str::contains("fn main() {}"))
        // Check that .gitignore is respected within the cloned repo.
        // The `ignore` crate should see the .gitignore and skip app.log.
        .stdout(predicate::str::contains("## File: data/app.log").not())
        // Check that uncommitted files are NOT included because they weren't cloned.
        .stdout(predicate::str::contains("## File: uncommitted.txt").not());

    // Restore original environment variable if it was set
    if let Ok(val) = original_xdg_cache_home {
        env::set_var("XDG_CACHE_HOME", val);
    } else {
        env::remove_var("XDG_CACHE_HOME");
    }

    Ok(())
}

/// Tests cloning a remote repository. This is a slow, network-dependent test.
/// It is ignored by default and should be run explicitly when testing network functionality.
/// To run: `cargo test -- --ignored git_input`
#[test]
#[ignore = "requires network access and is slow"]
fn test_git_remote_repository_input() -> Result<(), Box<dyn std::error::Error>> {
    // A small, public, and stable repository for testing purposes.
    let repo_url = "https://github.com/git-fixtures/basic.git";

    dircat_cmd()
        .arg(repo_url)
        .assert()
        .success()
        // Check for files known to be in the 'basic' fixture repo based on recent observation.
        .stdout(predicate::str::contains("## File: CHANGELOG"))
        .stdout(predicate::str::contains("Initial changelog"))
        .stdout(predicate::str::contains("## File: LICENSE"))
        .stdout(predicate::str::contains("Copyright (c) 2015 Tyba"))
        .stdout(predicate::str::contains("## File: go/example.go"))
        .stdout(predicate::str::contains("package harvesterd"));

    Ok(())
}
