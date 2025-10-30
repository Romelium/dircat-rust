// src/git.rs

use crate::config::Config;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository, ResetType};
use hex;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Determines the root cache directory for git repositories.
/// Typically `~/.cache/dircat/repos`.
fn get_base_cache_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "romelium", "dircat")
        .context("Could not determine project cache directory")?;
    let cache_dir = proj_dirs.cache_dir().join("repos");
    Ok(cache_dir)
}

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

/// Sets up remote callbacks for authentication and progress reporting.
fn create_remote_callbacks() -> RemoteCallbacks<'static> {
    // --- Setup Callbacks for Authentication and Progress ---
    let mut callbacks = RemoteCallbacks::new();

    // Authentication: Try SSH agent first, then default key paths.
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        let username = username_from_url.unwrap_or("git");
        log::debug!("Attempting SSH authentication for user: {}", username);

        // Try to authenticate with an SSH agent
        if let Ok(cred) = Cred::ssh_key_from_agent(username) {
            log::debug!("Authenticated via SSH agent");
            return Ok(cred);
        }

        // Fallback to default SSH key locations
        // This checks for ~/.ssh/id_rsa, etc.
        if let Ok(cred) = Cred::ssh_key(
            username,
            None,
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map(std::path::PathBuf::from)
                .ok()
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(""))
                .join(".ssh")
                .join("id_rsa")
                .as_path(),
            None,
        ) {
            log::debug!("Authenticated via default SSH key path");
            return Ok(cred);
        }

        log::warn!("SSH authentication failed: No agent or default keys found.");
        Err(git2::Error::from_str(
            "Authentication failed: could not connect with SSH agent or default keys",
        ))
    });

    // Progress reporting
    let progress_bar = {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb
    };

    // Clone the progress bar and move the clone into the closure.
    let pb_clone = progress_bar.clone();
    callbacks.transfer_progress(move |stats| {
        if stats.received_objects() == stats.total_objects() {
            pb_clone.set_length(stats.total_deltas() as u64);
            pb_clone.set_position(stats.indexed_deltas() as u64);
            pb_clone.set_message("Resolving deltas...");
        } else if stats.total_objects() > 0 {
            pb_clone.set_length(stats.total_objects() as u64);
            pb_clone.set_position(stats.received_objects() as u64);
            pb_clone.set_message("Receiving objects...");
        }
        true
    });

    callbacks
}

fn create_fetch_options(config: &Config) -> FetchOptions<'static> {
    // --- Setup Fetch Options ---
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(create_remote_callbacks());
    if let Some(depth) = config.git_depth {
        fetch_options.depth(depth as i32);
        log::debug!("Set shallow clone depth to: {}", depth);
    }

    fetch_options
}

/// Ensures the local repository is up-to-date with the remote.
/// Fetches from the remote and performs a hard reset to the remote branch head.
fn update_repo(repo: &Repository, config: &Config) -> Result<()> {
    log::info!("Updating cached repository...");
    let mut remote = repo.find_remote("origin")?;
    let mut fetch_options = create_fetch_options(config);

    // Fetch updates from the remote
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .context("Failed to fetch from remote 'origin'")?;

    let branch_name = config.git_branch.as_deref().unwrap_or("main"); // A reasonable default, though HEAD would be better
    let remote_branch_ref = format!("refs/remotes/origin/{}", branch_name);

    // Find the commit the remote branch is pointing to
    let fetch_head = repo
        .find_reference(&remote_branch_ref)
        .with_context(|| format!("Could not find remote branch '{}'", remote_branch_ref))?;
    let fetch_commit =
        repo.find_commit(fetch_head.target().context("Remote branch has no target")?)?;

    // Reset the local repository to match the fetched commit
    repo.reset(
        fetch_commit.as_object(),
        ResetType::Hard,
        None, // No checkout builder needed for hard reset
    )
    .context("Failed to perform hard reset on cached repository")?;

    log::info!("Cached repository updated successfully.");
    Ok(())
}

/// Retrieves a git repository, cloning it if not cached, or updating it if it is.
/// Returns the path to the up-to-date local repository.
pub(crate) fn get_repo_with_base_cache(
    base_cache_dir: &Path,
    url: &str,
    config: &Config,
) -> Result<PathBuf> {
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
                update_repo(&repo, config)?;
                return Ok(repo_path);
            }
            Err(e) => {
                log::warn!(
                    "Cached repository at '{}' is corrupted or invalid: {}. Re-cloning...",
                    repo_path.display(),
                    e
                );
                // Robustly remove the corrupted entry, whether it's a file or a directory.
                if repo_path.is_dir() {
                    fs::remove_dir_all(&repo_path).with_context(|| {
                        format!(
                            "Failed to remove corrupted cache directory at '{}'",
                            repo_path.display()
                        )
                    })?;
                } else if repo_path.is_file() {
                    fs::remove_file(&repo_path).with_context(|| {
                        format!(
                            "Failed to remove corrupted cache file at '{}'",
                            repo_path.display()
                        )
                    })?;
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
    fs::create_dir_all(&repo_path.parent().unwrap()).context("Failed to create cache directory")?;

    let fetch_options = create_fetch_options(config);
    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);
    if let Some(branch) = &config.git_branch {
        repo_builder.branch(branch);
    }

    repo_builder.clone(url, &repo_path)?;

    log::info!("Successfully cloned repository into cache.");
    Ok(repo_path)
}

/// Public-facing function to get a repo, using the system's standard cache directory.
pub fn get_repo(url: &str, config: &Config) -> Result<PathBuf> {
    let base_cache_dir = get_base_cache_dir()?;
    get_repo_with_base_cache(&base_cache_dir, url, config)
}

/// Checks if a given string is a likely git repository URL.
pub fn is_git_url(path_str: &str) -> bool {
    path_str.starts_with("https://")
        || path_str.starts_with("http://")
        || path_str.starts_with("git@")
        || path_str.starts_with("file://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
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
        let config = Config::new_for_test();

        // 1. Cache Miss
        let cached_path = get_repo_with_base_cache(cache_dir.path(), &remote_url, &config)?;
        assert!(cached_path.exists());
        let content = fs::read_to_string(cached_path.join("file.txt"))?;
        assert_eq!(content, "content v1");

        // 2. Cache Hit (no changes)
        let cached_path_2 = get_repo_with_base_cache(cache_dir.path(), &remote_url, &config)?;
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
        let config = Config::new_for_test();

        // 1. Initial clone
        let cached_path = get_repo_with_base_cache(cache_dir.path(), &remote_url, &config)?;
        assert_eq!(
            fs::read_to_string(cached_path.join("file.txt"))?,
            "content v1"
        );

        // 2. Update remote
        add_commit_to_repo(&remote_repo, "file.txt", "content v2", "Update")?;

        // 3. Fetch and update
        let updated_path = get_repo_with_base_cache(cache_dir.path(), &remote_url, &config)?;
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
        let config = Config::new_for_test();

        // 1. Manually create a corrupted cache entry (a file instead of a dir)
        let expected_cache_path = get_repo_cache_path(cache_dir.path(), &remote_url);
        fs::create_dir_all(expected_cache_path.parent().unwrap())?;
        File::create(&expected_cache_path)?.write_all(b"corruption")?;

        // 2. Attempt to get the repo. It should delete the file and re-clone.
        let cached_path = get_repo_with_base_cache(cache_dir.path(), &remote_url, &config)?;
        assert!(cached_path.is_dir()); // It's a directory now
        assert_eq!(fs::read_to_string(cached_path.join("file.txt"))?, "content");

        Ok(())
    }
}