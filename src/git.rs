// src/git.rs
//! Handles cloning, caching, and updating of git repositories provided as input.
//!
//! This module provides functionality to:
//! - Clone remote git repositories into a local cache.
//! - Update cached repositories on subsequent runs.
//! - Parse GitHub folder URLs and download their contents via the GitHub API.
//! - Provide authentication callbacks for SSH.

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository, ResetType};
use hex;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::env;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::Builder as TempDirBuilder;

/// Represents the components of a parsed GitHub folder URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGitUrl {
    /// The full URL to be used for cloning (e.g., `https://github.com/user/repo.git`).
    pub clone_url: String,
    /// The branch or tag name extracted from the URL. Can be "HEAD" as a placeholder for the default branch.
    pub branch: String,
    /// The path to the subdirectory within the repository. Can be empty if the URL points to the root.
    pub subdirectory: String,
}

/// Represents a file or directory item from the GitHub Contents API.
#[derive(Deserialize, Debug)]
struct ContentItem {
    path: String,
    #[serde(rename = "type")]
    item_type: String,
    download_url: Option<String>,
}

/// Represents the repository metadata from the GitHub API, only for getting the default branch.
#[derive(Deserialize, Debug)]
struct RepoInfo {
    default_branch: String,
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
    // Enable pruning to remove remote-tracking branches that no longer exist on the remote.
    fetch_options.prune(git2::FetchPrune::On);
    // Download all tags from the remote. This is important for checking out tags.
    fetch_options.download_tags(git2::AutotagOption::All);
    if let Some(depth) = config.git_depth {
        fetch_options.depth(depth as i32);
        log::debug!("Set shallow clone depth to: {}", depth);
    }

    fetch_options
}

/// Finds the target commit on the remote that the repository should be updated to.
///
/// It determines the target commit by:
/// 1. Using the branch specified by `--git-branch`.
/// 2. If no branch is specified, resolving the remote's `HEAD` to find the default branch.
fn find_remote_commit<'a>(repo: &'a Repository, config: &Config) -> Result<git2::Commit<'a>> {
    if let Some(ref_name) = &config.git_branch {
        log::debug!("Using user-specified ref: {}", ref_name);

        // 1. Try to resolve as a remote branch.
        let branch_ref_name = format!("refs/remotes/origin/{}", ref_name);
        if let Ok(reference) = repo.find_reference(&branch_ref_name) {
            log::debug!("Resolved '{}' as a remote branch.", ref_name);
            return repo
                .find_commit(
                    reference
                        .target()
                        .context("Remote branch reference has no target commit")?,
                )
                .context("Failed to find commit for branch reference");
        }

        // 2. Try to resolve as a tag.
        let tag_ref_name = format!("refs/tags/{}", ref_name);
        if let Ok(reference) = repo.find_reference(&tag_ref_name) {
            log::debug!("Resolved '{}' as a tag.", ref_name);
            // A tag can be lightweight (points directly to a commit) or annotated
            // (points to a tag object, which then points to a commit).
            // `peel_to_commit` handles both cases.
            let object = reference.peel(git2::ObjectType::Commit)?;
            return object
                .into_commit()
                .map_err(|_| anyhow!("Tag '{}' does not point to a commit", ref_name));
        }

        // 3. If both fail, return a comprehensive error.
        return Err(anyhow!(
            "Could not find remote branch or tag named '{}' after fetch. Does this ref exist on the remote?",
            ref_name
        ));
    }

    // User did not specify a branch, so find the remote's default by resolving its HEAD.
    log::debug!("Resolving remote's default branch via origin/HEAD");
    let remote_head = repo.find_reference("refs/remotes/origin/HEAD").context(
        "Could not find remote's HEAD. The repository might not have a default branch set, or it may be empty. Please specify a branch with --git-branch.",
    )?;
    let remote_branch_ref_name = remote_head
        .symbolic_target()
        .context("Remote HEAD is not a symbolic reference; cannot determine default branch.")?
        .to_string();

    log::debug!("Targeting remote reference: {}", remote_branch_ref_name);
    let fetch_head = repo.find_reference(&remote_branch_ref_name).with_context(|| {
        format!(
            "Could not find remote branch reference '{}' after fetch. Does this branch exist on the remote?",
            remote_branch_ref_name
        )
    })?;
    repo.find_commit(
        fetch_head
            .target()
            .context("Remote branch reference has no target commit")?,
    )
    .context("Failed to find commit for default branch reference")
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

    // Find the specific commit we need to reset to.
    let target_commit = find_remote_commit(repo, config)?;
    // Detach HEAD before resetting to avoid issues with checked-out branches.
    repo.set_head_detached(target_commit.id())
        .context("Failed to detach HEAD in cached repository")?;

    // Reset the local repository to match the fetched commit
    repo.reset(
        target_commit.as_object(),
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
    fs::create_dir_all(repo_path.parent().unwrap()).context("Failed to create cache directory")?;

    let fetch_options = create_fetch_options(config);
    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);

    // If a specific ref is given that might be a tag, we cannot use `repo_builder.branch()`.
    // Instead, we clone the default branch first, and then find and check out the specific ref.
    // This is more robust and handles both branches and tags correctly on initial clone.
    if let Some(ref_name) = &config.git_branch {
        log::debug!(
            "Cloning default branch first, will check out '{}' after.",
            ref_name
        );
    }

    let repo = repo_builder
        .clone(url, &repo_path)
        .context("Failed to clone repository")?;
    log::info!("Successfully cloned repository into cache.");

    // If a specific branch/tag was requested, we need to check it out now.
    // The clone operation by default only checks out the remote's HEAD.
    if config.git_branch.is_some() {
        log::info!(
            "Checking out specified ref: {:?}",
            config.git_branch.as_ref().unwrap()
        );
        // Find the commit associated with the branch or tag.
        let target_commit = find_remote_commit(&repo, config)?;

        // Detach HEAD and reset the working directory to the target commit.
        repo.set_head_detached(target_commit.id())
            .context("Failed to detach HEAD in newly cloned repository")?;
        repo.reset(target_commit.as_object(), ResetType::Hard, None)
            .context("Failed to perform hard reset on newly cloned repository")?;
        log::info!("Successfully checked out specified ref.");
    }

    Ok(repo_path)
}

/// Public-facing function to get a repo, using the configured cache directory.
///
/// This function will clone a remote git repository into a local cache directory.
/// If the repository is already cached, it will fetch updates from the remote.
///
/// # Arguments
/// * `url` - The URL of the git repository to clone.
/// * `config` - The application configuration, containing cache path and git options.
///
/// # Returns
/// A `Result` containing the `PathBuf` to the local, up-to-date repository on success.
///
/// # Errors
/// Returns an error if cloning or updating the repository fails.
pub fn get_repo(url: &str, config: &Config) -> Result<PathBuf> {
    let base_cache_dir = &config.git_cache_path;
    get_repo_with_base_cache(base_cache_dir, url, config)
}

/// Checks if a given string is a likely git repository URL.
///
/// This is a simple heuristic check and does not validate the URL's format.
///
/// # Examples
/// ```
/// use dircat::git::is_git_url;
///
/// assert!(is_git_url("https://github.com/user/repo.git"));
/// assert!(is_git_url("git@github.com:user/repo.git"));
/// assert!(!is_git_url("/local/path/to/repo"));
/// ```
pub fn is_git_url(path_str: &str) -> bool {
    path_str.starts_with("https://")
        || path_str.starts_with("http://")
        || path_str.starts_with("git@")
        || path_str.starts_with("file://")
}

/// Regex for GitHub folder URLs: `.../tree/branch/path`
static GITHUB_TREE_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https://github\.com/([^/]+)/([^/]+)/tree/([^/]+)(?:/(.*))?$").unwrap()
});

/// Parses a GitHub folder URL into its constituent parts.
///
/// Handles the official format (`.../tree/branch/path`) as well as common "sloppy" formats
/// like `.../branch/path` or just `.../path` (which assumes the default branch).
///
/// # Returns
/// `Some(ParsedGitUrl)` if the URL is a parsable GitHub folder URL, otherwise `None`.
///
/// # Examples
/// ```
/// use dircat::git::{parse_github_folder_url, ParsedGitUrl};
///
/// let url = "https://github.com/rust-lang/cargo/tree/master/src/cargo";
/// let parsed = parse_github_folder_url(url).unwrap();
///
/// assert_eq!(parsed, ParsedGitUrl {
///     clone_url: "https://github.com/rust-lang/cargo.git".to_string(),
///     branch: "master".to_string(),
///     subdirectory: "src/cargo".to_string(),
/// });
///
/// // A "sloppy" URL without the /tree/ part assumes the default branch.
/// let sloppy_url = "https://github.com/rust-lang/cargo/src/cargo";
/// let sloppy_parsed = parse_github_folder_url(sloppy_url).unwrap();
///
/// assert_eq!(sloppy_parsed, ParsedGitUrl {
///     clone_url: "https://github.com/rust-lang/cargo.git".to_string(),
///     branch: "HEAD".to_string(), // "HEAD" indicates default branch
///     subdirectory: "src/cargo".to_string(),
/// });
///
/// // A root repository URL is not a folder URL.
/// assert!(parse_github_folder_url("https://github.com/rust-lang/cargo").is_none());
/// ```
pub fn parse_github_folder_url(url: &str) -> Option<ParsedGitUrl> {
    // 1. Try the official, correct format first.
    if let Some(caps) = GITHUB_TREE_URL_RE.captures(url) {
        let user = caps.get(1).unwrap().as_str();
        let repo = caps.get(2).unwrap().as_str();
        let branch = caps.get(3).unwrap().as_str();
        // Get the subdirectory path, or an empty string if it's not present.
        // Trim any trailing slash for consistency.
        let subdirectory = caps.get(4).map_or("", |m| m.as_str()).trim_end_matches('/');

        return Some(ParsedGitUrl {
            clone_url: format!("https://github.com/{}/{}.git", user, repo),
            branch: branch.to_string(),
            subdirectory: subdirectory.to_string(),
        });
    }

    // 2. If not, try to parse it as a "sloppy" URL.
    let path_part = url.strip_prefix("https://github.com/")?;

    let parts: Vec<&str> = path_part.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() < 3 {
        // Not enough parts for user/repo/path, so it's a root URL or invalid.
        return None;
    }

    let user = parts[0];
    let repo = parts[1].trim_end_matches(".git"); // Handle optional .git suffix
    let first_segment = parts[2];

    // Check for reserved GitHub path names to avoid misinterpreting e.g. .../issues/123
    let reserved_names = [
        "releases", "tags", "pull", "issues", "actions", "projects", "wiki", "security", "pulse",
        "graphs", "settings", "blob", "tree", "commit", "blame", "find",
    ];
    if reserved_names.contains(&first_segment) {
        return None;
    }

    // For sloppy URLs (without `/tree/`), we cannot reliably determine the branch from the path,
    // as a branch name is indistinguishable from a directory name (e.g., 'main' could be a
    // branch or a folder).
    // To ensure predictable behavior, we assume the entire path after user/repo is the
    // subdirectory and that the user wants the default branch ('HEAD').
    // If a different branch is desired, the user should use the correct `/tree/BRANCH/` URL
    // or specify the branch with the `--git-branch` flag.
    let branch = "HEAD";
    let subdirectory = parts[2..].join("/");

    Some(ParsedGitUrl {
        clone_url: format!("https://github.com/{}/{}.git", user, repo),
        branch: branch.to_string(),
        subdirectory,
    })
}

/// Downloads a directory's contents from the GitHub API into a temporary directory.
///
/// This is much faster than a full `git clone` for large repositories. It recursively
/// lists and downloads all files within the specified subdirectory.
///
/// # Arguments
/// * `url_parts` - A `ParsedGitUrl` struct containing the repository and path information.
/// * `config` - The application configuration, used for resolving the branch if needed.
///
/// # Returns
/// A `Result` containing the `PathBuf` to the temporary directory where files were downloaded.
/// The caller is responsible for managing the lifetime of this directory.
///
/// # Errors
/// Returns an error if API requests fail, the directory is not found, or file I/O fails.
/// If a `GITHUB_TOKEN` environment variable is not set, this may fail due to API rate limiting.
pub fn download_directory_via_api(url_parts: &ParsedGitUrl, config: &Config) -> Result<PathBuf> {
    // 1. Setup
    let temp_dir = TempDirBuilder::new().prefix("dircat-git-api-").tempdir()?;
    let client = build_reqwest_client()?;
    let (owner, repo) = parse_clone_url(&url_parts.clone_url)?;

    // 2. Resolve branch
    let branch_to_use = if let Some(cli_branch) = &config.git_branch {
        // Always prioritize the branch specified on the command line.
        log::debug!("Using branch from --git-branch flag: {}", cli_branch);
        cli_branch.clone()
    } else if url_parts.branch != "HEAD" {
        // Otherwise, use the branch from the URL if it's not a root URL.
        log::debug!("Using branch from URL: {}", url_parts.branch);
        url_parts.branch.clone()
    } else {
        // Finally, fall back to the repository's default branch.
        log::debug!("Fetching default branch for {}/{}", owner, repo);
        fetch_default_branch(&owner, &repo, &client)?
    };
    log::info!("Processing repository on branch: {}", branch_to_use);

    // 3. List all files
    let files_to_download =
        list_all_files_recursively(&client, &owner, &repo, &branch_to_use, url_parts)?;

    if files_to_download.is_empty() {
        // Leak the TempDir to prevent it from being deleted, and return its path.
        return Ok(temp_dir.keep());
    }

    // 4. Download files in parallel
    use rayon::prelude::*;
    files_to_download
        .par_iter()
        .map(|file_item| download_and_write_file(&client, file_item, temp_dir.path()))
        .collect::<Result<()>>()?;

    // 5. Return path to temp dir, consuming the TempDir object to prevent deletion.
    // Leak the TempDir to prevent it from being deleted, and return its path.
    Ok(temp_dir.keep())
}

/// Builds a `reqwest` client with default headers for GitHub API interaction.
fn build_reqwest_client() -> Result<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(ACCEPT, "application/vnd.github.v3+json".parse()?);
    headers.insert(USER_AGENT, "dircat-rust-downloader".parse()?);

    if let Ok(token) = env::var("GITHUB_TOKEN") {
        headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse()?);
        log::debug!("Using GITHUB_TOKEN for authentication.");
    }

    let client = Client::builder().default_headers(headers).build()?;
    Ok(client)
}

/// Fetches the default branch name for a repository.
fn fetch_default_branch(owner: &str, repo: &str, client: &Client) -> Result<String> {
    let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    log::debug!("Fetching repo metadata from: {}", api_url);
    let response = client.get(&api_url).send()?.error_for_status()?;
    let repo_info: RepoInfo = response.json()?;
    Ok(repo_info.default_branch)
}

/// Recursively lists all files in a given GitHub directory path using a queue.
fn list_all_files_recursively(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
    url_parts: &ParsedGitUrl,
) -> Result<Vec<ContentItem>> {
    let mut files = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(url_parts.subdirectory.clone());

    while let Some(path) = queue.pop_front() {
        let api_url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
            owner, repo, path, branch
        );

        log::debug!("Fetching directory contents from: {}", api_url);
        let response = client.get(&api_url).send()?.error_for_status()?;

        // The API returns a single object if the path is a file, or an array for a directory.
        let response_text = response.text()?;
        let json_value: Value = serde_json::from_str(&response_text)?;

        let items: Vec<ContentItem> = if json_value.is_array() {
            serde_json::from_value(json_value)?
        } else if json_value.is_object() {
            vec![serde_json::from_value(json_value)?]
        } else {
            vec![]
        };

        for item in items {
            if item.item_type == "file" {
                if item.download_url.is_some() {
                    files.push(item);
                } else {
                    log::warn!("Skipping file with no download_url: {}", item.path);
                }
            } else if item.item_type == "dir" {
                queue.push_back(item.path);
            }
        }
    }
    Ok(files)
}

/// Downloads a single file and writes it to the correct relative path in the base directory.
fn download_and_write_file(
    client: &Client,
    file_item: &ContentItem,
    base_dir: &Path,
) -> Result<()> {
    let download_url = file_item.download_url.as_ref().unwrap(); // We already filtered for Some
    log::debug!("Downloading file from: {}", download_url);

    let response = client.get(download_url).send()?.error_for_status()?;
    let content = response.bytes()?;

    let local_path = base_dir.join(&file_item.path);
    if let Some(parent_dir) = local_path.parent() {
        fs::create_dir_all(parent_dir).with_context(|| {
            format!(
                "Failed to create directory structure for '{}'",
                local_path.display()
            )
        })?;
    }

    fs::write(&local_path, content).with_context(|| {
        format!(
            "Failed to write downloaded content to '{}'",
            local_path.display()
        )
    })
}

/// Helper to get owner/repo from a clone URL like "https://github.com/user/repo.git"
fn parse_clone_url(clone_url: &str) -> Result<(String, String)> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"github\.com[/:]([^/]+)/([^/]+?)(?:\.git)?$").unwrap());
    RE.captures(clone_url)
        .and_then(|caps| Some((caps.get(1)?.as_str(), caps.get(2)?.as_str())))
        .map(|(owner, repo)| (owner.to_string(), repo.to_string()))
        .ok_or_else(|| anyhow!("Could not parse owner/repo from clone URL: {}", clone_url))
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

    #[test]
    fn test_parse_github_folder_url_valid() {
        let url = "https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore";
        let expected = Some(ParsedGitUrl {
            clone_url: "https://github.com/BurntSushi/ripgrep.git".to_string(),
            branch: "master".to_string(),
            subdirectory: "crates/ignore".to_string(),
        });
        assert_eq!(parse_github_folder_url(url), expected);
    }

    #[test]
    fn test_parse_github_sloppy_url_no_tree_assumes_default_branch() {
        // Missing 'tree' part: .../user/repo/branch/path
        // The parser treats 'master' as part of the path and assumes the default branch ('HEAD').
        let url = "https://github.com/BurntSushi/ripgrep/master/crates/ignore";
        let expected = Some(ParsedGitUrl {
            clone_url: "https://github.com/BurntSushi/ripgrep.git".to_string(),
            branch: "HEAD".to_string(), // Assumes default branch
            subdirectory: "master/crates/ignore".to_string(), // 'master' is part of the path
        });
        assert_eq!(parse_github_folder_url(url), expected);
    }

    #[test]
    fn test_parse_github_sloppy_url_no_branch() {
        // Missing 'tree/branch' part: .../user/repo/path
        let url = "https://github.com/BurntSushi/ripgrep/crates/ignore";
        let expected = Some(ParsedGitUrl {
            clone_url: "https://github.com/BurntSushi/ripgrep.git".to_string(),
            branch: "HEAD".to_string(),
            subdirectory: "crates/ignore".to_string(),
        });
        assert_eq!(parse_github_folder_url(url), expected);
    }

    #[test]
    fn test_parse_github_sloppy_url_with_git_suffix() {
        // Sloppy URL with .git in the repo part
        let url = "https://github.com/BurntSushi/ripgrep.git/master/crates/ignore";
        let expected = Some(ParsedGitUrl {
            clone_url: "https://github.com/BurntSushi/ripgrep.git".to_string(),
            branch: "HEAD".to_string(),
            subdirectory: "master/crates/ignore".to_string(),
        });
        assert_eq!(parse_github_folder_url(url), expected);
    }

    #[test]
    fn test_parse_github_url_rejects_root() {
        assert_eq!(
            parse_github_folder_url("https://github.com/rust-lang/rust"),
            None
        );
        assert_eq!(
            parse_github_folder_url("https://github.com/rust-lang/rust.git"),
            None
        );
    }

    #[test]
    fn test_parse_github_url_rejects_reserved_paths() {
        assert_eq!(
            parse_github_folder_url("https://github.com/user/repo/blob/master/file.txt"),
            None
        );
        assert_eq!(
            parse_github_folder_url("https://github.com/user/repo/issues/1"),
            None
        );
        assert_eq!(
            parse_github_folder_url("https://github.com/user/repo/pull/2"),
            None
        );
        assert_eq!(
            parse_github_folder_url("https://gitlab.com/user/repo/tree/master"),
            None
        );
    }
}
