// src/git/clone.rs
//! Handles cloning, caching, and updating of git repositories using `git2`.

use crate::errors::GitError;
use crate::progress::ProgressReporter;
use anyhow::{Context, Result};
use git2::{build::RepoBuilder, Repository};
use hex;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::ops::{create_fetch_options, find_remote_commit, update_repo};

/// Gets the specific cache directory path for a given repository URL within a base directory.
/// The final path is `<base_cache_dir>/<sha256_of_url>`.
fn get_repo_cache_path(base_cache_dir: &Path, url: &str) -> PathBuf {
    // Create a unique, filesystem-safe directory name from the URL
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    let hex_hash = hex::encode(hash);

    base_cache_dir.join(hex_hash)
}

/// Retrieves a git repository, cloning it if not cached, or updating it if it is.
/// Returns the path to the up-to-date local repository.
fn get_repo_with_base_cache(
    base_cache_dir: &Path,
    url: &str,
    branch: &Option<String>,
    depth: Option<u32>,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<PathBuf, GitError> {
    let repo_path = get_repo_cache_path(base_cache_dir, url);

    if repo_path.exists() {
        log::info!(
            "Found cached repository for '{}' at '{}'. Checking for updates...",
            url,
            repo_path.display()
        );
        match Repository::open(&repo_path) {
            Ok(repo) => {
                // Repo exists and is valid, update it
                update_repo(&repo, branch, depth, progress.clone())?;
                if let Some(p) = &progress {
                    p.finish_with_message("Update complete.".to_string());
                }
                return Ok(repo_path);
            }
            Err(_) => {
                log::warn!(
                    "Cached repository at '{}' is corrupted or invalid. Re-cloning...",
                    repo_path.display(),
                );
                // Robustly remove the corrupted entry, whether it's a file or a directory.
                if repo_path.is_dir() {
                    fs::remove_dir_all(&repo_path)
                        .with_context(|| {
                            format!(
                                "Failed to remove corrupted cache directory at '{}'",
                                repo_path.display()
                            )
                        })
                        .map_err(GitError::Generic)?;
                } else if repo_path.is_file() {
                    fs::remove_file(&repo_path)
                        .with_context(|| {
                            format!(
                                "Failed to remove corrupted cache file at '{}'",
                                repo_path.display()
                            )
                        })
                        .map_err(GitError::Generic)?;
                }
            }
        }
    }

    // --- Cache Miss or Corrupted Cache ---
    log::info!(
        "Cloning git repository from '{}' into cache at '{}'...",
        url,
        repo_path.display()
    );
    fs::create_dir_all(repo_path.parent().unwrap())
        .context("Failed to create cache directory")
        .map_err(GitError::Generic)?;

    let fetch_options = create_fetch_options(depth, progress.clone());
    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);

    // If a specific ref is given that might be a tag, we cannot use `repo_builder.branch()`.
    // Instead, we clone the default branch first, and then find and check out the specific ref.
    // This is more robust and handles both branches and tags correctly on initial clone.
    if let Some(ref_name) = branch {
        log::debug!(
            "Cloning default branch first, will check out '{}' after.",
            ref_name
        );
    }

    let repo = repo_builder
        .clone(url, &repo_path)
        .map_err(|e| GitError::CloneFailed {
            url: url.to_string(),
            source: e,
        })?;
    if let Some(p) = &progress {
        p.finish_with_message("Clone complete.".to_string());
    }
    log::info!("Successfully cloned repository into cache.");

    // If a specific branch/tag was requested, we need to check it out now.
    // The clone operation by default only checks out the remote's HEAD.
    if branch.is_some() {
        log::info!("Checking out specified ref: {:?}", branch.as_ref().unwrap());
        // Find the commit associated with the branch or tag.
        let target_commit = find_remote_commit(&repo, branch)?;

        // Detach HEAD and reset the working directory to the target commit.
        repo.set_head_detached(target_commit.id())
            .context("Failed to detach HEAD in newly cloned repository")
            .map_err(|e| GitError::UpdateFailed(e.to_string()))?;
        repo.reset(target_commit.as_object(), git2::ResetType::Hard, None)
            .context("Failed to perform hard reset on newly cloned repository")
            .map_err(|e| GitError::UpdateFailed(e.to_string()))?;
        log::info!("Successfully checked out specified ref.");
    }

    Ok(repo_path)
}

/// Clones or updates a git repository into a local cache directory.
///
/// This function will clone a remote git repository into a specified local cache directory.
/// If the repository is already cached, it will fetch updates from the remote to ensure it is up-to-date.
///
/// # Arguments
/// * `url` - The URL of the git repository to clone.
/// * `branch` - An optional specific git branch or tag to check out. If `None`, the remote's default branch is used.
/// * `depth` - An optional depth for a shallow clone.
/// * `cache_path` - The absolute path to the base directory for caching repositories. A subdirectory will be created inside this path.
/// * `progress` - An optional progress reporter for long operations like cloning.
///
/// # Returns
/// A `Result` containing the `PathBuf` to the local, up-to-date repository on success.
///
/// # Errors
/// Returns a `GitError` if cloning or updating the repository fails.
pub fn get_repo(
    url: &str,
    branch: &Option<String>,
    depth: Option<u32>,
    cache_path: &Path,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<PathBuf, GitError> {
    get_repo_with_base_cache(cache_path, url, branch, depth, progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexTime, Signature};
    use std::fs::File;
    use std::io::Write;
    use tempfile::{tempdir, TempDir};

    /// Helper to create a bare git repo to act as a "remote"
    fn setup_test_remote_repo() -> Result<(TempDir, Repository)> {
        let remote_dir = tempdir()?;
        let repo = Repository::init_bare(remote_dir.path())?;
        Ok((remote_dir, repo))
    }

    /// Helper to add a commit to a bare repo
    fn add_commit_to_repo(
        repo: &Repository,
        filename: &str,
        content: &str,
        message: &str,
    ) -> Result<()> {
        let mut index = repo.index()?;
        let oid = repo.blob(content.as_bytes())?;
        let entry = git2::IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: content.len() as u32,
            id: oid,
            flags: 0,
            flags_extended: 0,
            path: filename.as_bytes().to_vec(),
        };
        index.add(&entry)?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let signature = Signature::now("Test", "test@example.com")?;
        let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;
        Ok(())
    }

    #[test]
    fn test_cache_miss_and_hit() -> Result<()> {
        let (_remote_dir, remote_repo) = setup_test_remote_repo()?;
        add_commit_to_repo(&remote_repo, "file.txt", "content v1", "Initial")?;
        let remote_path_str = _remote_dir.path().to_str().unwrap();
        #[cfg(windows)]
        let remote_url = format!("file:///{}", remote_path_str.replace('\\', "/"));
        #[cfg(not(windows))]
        let remote_url = format!("file://{}", remote_path_str);

        let cache_dir = tempdir()?;

        // 1. Cache Miss
        let cached_path =
            get_repo_with_base_cache(cache_dir.path(), &remote_url, &None, None, None)?;
        assert!(cached_path.exists());
        let content = fs::read_to_string(cached_path.join("file.txt"))?;
        assert_eq!(content, "content v1");

        // 2. Cache Hit (no changes)
        let cached_path_2 =
            get_repo_with_base_cache(cache_dir.path(), &remote_url, &None, None, None)?;
        assert_eq!(cached_path, cached_path_2); // Should be the same path
        let content_2 = fs::read_to_string(cached_path_2.join("file.txt"))?;
        assert_eq!(content_2, "content v1");

        Ok(())
    }

    #[test]
    fn test_cache_update() -> Result<()> {
        let (_remote_dir, remote_repo) = setup_test_remote_repo()?;
        add_commit_to_repo(&remote_repo, "file.txt", "content v1", "Initial")?;
        let remote_path_str = _remote_dir.path().to_str().unwrap();
        #[cfg(windows)]
        let remote_url = format!("file:///{}", remote_path_str.replace('\\', "/"));
        #[cfg(not(windows))]
        let remote_url = format!("file://{}", remote_path_str);

        let cache_dir = tempdir()?;

        // 1. Initial clone
        let cached_path =
            get_repo_with_base_cache(cache_dir.path(), &remote_url, &None, None, None)?;
        assert_eq!(
            fs::read_to_string(cached_path.join("file.txt"))?,
            "content v1"
        );

        // 2. Update remote
        add_commit_to_repo(&remote_repo, "file.txt", "content v2", "Update")?;

        // 3. Fetch and update
        let updated_path =
            get_repo_with_base_cache(cache_dir.path(), &remote_url, &None, None, None)?;
        assert_eq!(cached_path, updated_path);
        assert_eq!(
            fs::read_to_string(updated_path.join("file.txt"))?,
            "content v2"
        );

        Ok(())
    }

    #[test]
    fn test_corrupted_cache_recovery() -> Result<()> {
        let (_remote_dir, remote_repo) = setup_test_remote_repo()?;
        add_commit_to_repo(&remote_repo, "file.txt", "content", "Initial")?;
        let remote_path_str = _remote_dir.path().to_str().unwrap();
        #[cfg(windows)]
        let remote_url = format!("file:///{}", remote_path_str.replace('\\', "/"));
        #[cfg(not(windows))]
        let remote_url = format!("file://{}", remote_path_str);

        let cache_dir = tempdir()?;

        // 1. Manually create a corrupted cache entry (a file instead of a dir)
        let expected_cache_path = get_repo_cache_path(cache_dir.path(), &remote_url);
        fs::create_dir_all(expected_cache_path.parent().unwrap())?;
        File::create(&expected_cache_path)?.write_all(b"corruption")?;

        // 2. Attempt to get the repo. It should delete the file and re-clone.
        let cached_path =
            get_repo_with_base_cache(cache_dir.path(), &remote_url, &None, None, None)?;
        assert!(cached_path.is_dir()); // It's a directory now
        assert_eq!(fs::read_to_string(cached_path.join("file.txt"))?, "content");

        Ok(())
    }
}
