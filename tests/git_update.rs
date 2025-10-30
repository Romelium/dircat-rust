// tests/git_update.rs

mod common;

use anyhow::Result;
use assert_cmd::prelude::*;
use common::dircat_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::{tempdir, TempDir};

// --- Test Setup Helpers ---

/// A helper struct to manage a temporary "remote" BARE git repository for testing.
struct TestRemote {
    _temp_dir: TempDir,
    repo: git2::Repository,
    url: String,
}

impl TestRemote {
    /// Creates a new, empty BARE git repository in a temporary directory.
    fn new() -> Result<Self> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path();
        let repo = git2::Repository::init_bare(repo_path)?;

        #[cfg(windows)]
        let url = format!("file:///{}", repo_path.to_str().unwrap().replace('\\', "/"));
        #[cfg(not(windows))]
        let url = format!("file://{}", repo_path.to_str().unwrap());

        Ok(Self {
            _temp_dir: temp_dir,
            repo,
            url,
        })
    }

    /// Adds a file with content and commits it to a specific branch in the bare repo.
    fn commit_file(&self, branch: &str, path: &str, content: &str, msg: &str) -> Result<()> {
        let signature = git2::Signature::now("Test User", "test@example.com")?;

        // Find parent commit and its tree
        let parent_commit = self
            .repo
            .find_branch(branch, git2::BranchType::Local)
            .ok()
            .and_then(|b| b.get().peel_to_commit().ok());
        let parent_tree = parent_commit.as_ref().and_then(|c| c.tree().ok());

        // Create a tree builder based on the parent's tree, or an empty one
        let mut tree_builder = if let Some(tree) = &parent_tree {
            self.repo.treebuilder(Some(tree))?
        } else {
            self.repo.treebuilder(None)?
        };

        // Create a blob for the new file content
        let blob_oid = self.repo.blob(content.as_bytes())?;
        // Insert the file into the tree builder
        tree_builder.insert(path, blob_oid, 0o100644)?;

        // Write the new tree to the object database
        let tree_oid = tree_builder.write()?;
        let tree = self.repo.find_tree(tree_oid)?;

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

        // Create the commit
        let commit_oid = self.repo.commit(
            None, // Don't update HEAD here
            &signature, &signature, msg, &tree, &parents,
        )?;
        let commit = self.repo.find_commit(commit_oid)?;

        // Update or create the branch to point to the new commit.
        self.repo.branch(branch, &commit, true)?;
        Ok(())
    }

    /// Sets the default branch (HEAD) of the repository.
    fn set_default_branch(&self, branch: &str) -> Result<()> {
        self.repo.set_head(&format!("refs/heads/{}", branch))?;
        Ok(())
    }

    /// Deletes a local branch.
    fn delete_branch(&self, branch: &str) -> Result<()> {
        let mut branch_ref = self.repo.find_branch(branch, git2::BranchType::Local)?;
        branch_ref.delete()?;
        Ok(())
    }
}

// --- Tests ---

#[test]
fn test_git_update_with_different_default_branch() -> Result<()> {
    let remote = TestRemote::new()?;
    remote.commit_file("master", "file.txt", "v1", "Initial commit")?;
    remote.set_default_branch("master")?;
    let cache_dir = tempdir()?;

    // 1. First run: clones the repo.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .assert()
        .success()
        .stdout(predicate::str::contains("v1"));

    // 2. Update the remote.
    remote.commit_file("master", "file.txt", "v2", "Update file")?;

    // 3. Second run: should find the cache, fetch, and update to 'master'.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .assert()
        .success()
        .stdout(predicate::str::contains("v2"))
        .stdout(predicate::str::contains("v1").not());

    Ok(())
}

#[test]
fn test_git_update_with_specific_branch() -> Result<()> {
    let remote = TestRemote::new()?;
    remote.commit_file("main", "main.txt", "main v1", "main commit")?;
    remote.commit_file("develop", "dev.txt", "dev v1", "develop commit")?;
    remote.set_default_branch("main")?;
    let cache_dir = tempdir()?;

    // 1. First run: clones the 'develop' branch specifically.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .arg("--git-branch")
        .arg("develop")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev v1"))
        .stdout(predicate::str::contains("main v1").not());

    // 2. Update the 'develop' branch on the remote.
    remote.commit_file("develop", "dev.txt", "dev v2", "Update develop")?;

    // 3. Second run: should update the 'develop' branch.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .arg("--git-branch")
        .arg("develop")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev v2"))
        .stdout(predicate::str::contains("dev v1").not());

    Ok(())
}

#[test]
fn test_git_update_prunes_deleted_branch() -> Result<()> {
    let remote = TestRemote::new()?;
    remote.commit_file("main", "main.txt", "v1", "main commit")?;
    remote.commit_file("feature", "feature.txt", "feature content", "feat commit")?;
    remote.set_default_branch("main")?;

    // 1. First run: clones the repo, cache will have 'main' and 'feature'.
    let cache_dir = tempdir()?;
    let repos_base_dir = cache_dir.path();
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", repos_base_dir)
        .arg(&remote.url)
        .assert()
        .success();

    // Verify the feature branch exists in the cache.
    let cached_repo_path = fs::read_dir(repos_base_dir)?.next().unwrap()?.path();
    let cached_repo = git2::Repository::open(&cached_repo_path)?;
    assert!(cached_repo
        .find_reference("refs/remotes/origin/feature")
        .is_ok());

    // 2. Delete the branch on the remote.
    remote.delete_branch("feature")?;

    // 3. Second run: should fetch with prune enabled.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", repos_base_dir)
        .arg(&remote.url)
        .assert()
        .success();

    // 4. Verify the feature branch has been pruned from the cache.
    let cached_repo_reopened = git2::Repository::open(&cached_repo_path)?;
    assert!(cached_repo_reopened
        .find_reference("refs/remotes/origin/feature")
        .is_err());

    Ok(())
}

#[test]
fn test_git_update_non_existent_branch() -> Result<()> {
    let remote = TestRemote::new()?;
    remote.commit_file("main", "main.txt", "v1", "main commit")?;
    remote.set_default_branch("main")?;
    let cache_dir = tempdir()?;

    // 1. First run: clones 'main' successfully.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .assert()
        .success();

    // 2. Second run: attempts to update a branch that doesn't exist.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .arg("--git-branch")
        .arg("ghost-branch")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Could not find remote branch or tag named 'ghost-branch' after fetch.",
        ));

    Ok(())
}

#[test]
fn test_git_update_to_specific_tag() -> Result<()> {
    let remote = TestRemote::new()?;
    // Commit v1 and tag it
    remote.commit_file("main", "file.txt", "v1 content", "Commit v1")?;
    // Find the commit we just created by looking at the 'main' branch tip.
    // This is more robust than using repo.head(), which might point to an
    // unborn 'master' branch in a new bare repository, causing failures on some platforms.
    let commit_v1 = remote.repo.find_branch("main", git2::BranchType::Local)?.get().peel_to_commit()?;

    remote.repo.tag(
        "v1.0",
        commit_v1.as_object(),
        &git2::Signature::now("Test User", "test@example.com")?,
        "Tag v1.0",
        false,
    )?;
    // Commit v2 to the same branch
    remote.commit_file("main", "file.txt", "v2 content", "Commit v2")?;
    remote.set_default_branch("main")?;
    let cache_dir = tempdir()?;

    // 1. First run: clones the repo, which will be on the 'main' branch (v2 content).
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .assert()
        .success()
        .stdout(predicate::str::contains("v2 content"));

    // 2. Second run: should find the cache, fetch tags, and update to the 'v1.0' tag.
    dircat_cmd()
        .env("DIRCAT_TEST_CACHE_DIR", cache_dir.path())
        .arg(&remote.url)
        .arg("--git-branch")
        .arg("v1.0")
        .assert()
        .success()
        .stdout(predicate::str::contains("v1 content"))
        .stdout(predicate::str::contains("v2 content").not());

    Ok(())
}
