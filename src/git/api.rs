// src/git/api.rs
//! Handles downloading directory contents from the GitHub API.

use super::url::{parse_clone_url, ParsedGitUrl};
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::Builder as TempDirBuilder;

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

/// Downloads a directory's contents from the GitHub API into a new temporary directory.
///
/// This is much faster than a full `git clone` for large repositories. It recursively
/// lists and downloads all files within the specified subdirectory.
///
/// To access private repositories or avoid API rate limits, set a `GITHUB_TOKEN`
/// environment variable with a Personal Access Token that has `repo` scope.
///
/// # Parameters
/// * `url_parts` - A [`ParsedGitUrl`] struct containing the repository and path information.
/// * `branch_override` - An optional branch name which, if provided, takes precedence over the branch specified in the URL.
///
/// # Returns
/// A `Result` containing the `PathBuf` to a new temporary directory where files were downloaded.
/// **Note:** The temporary directory is intentionally leaked (not automatically deleted) and the caller
/// is responsible for its cleanup if necessary.
///
/// # Errors
/// Returns an error if API requests fail, the directory is not found, or file I/O fails.
///
/// # Examples
/// ```no_run
/// use dircat::git::{parse_github_folder_url, download_directory_via_api};
/// # use anyhow::Result;
///
/// # fn main() -> Result<()> {
/// let url = "https://github.com/rust-lang/cargo/tree/master/src/cargo";
/// if let Some(parsed_url) = parse_github_folder_url(url) {
///     // Download the 'src/cargo' directory from the 'master' branch.
///     let temp_dir_path = download_directory_via_api(&parsed_url, &None)?;
///     println!("Downloaded to: {}", temp_dir_path.display());
///     // Remember to clean up the temp directory if needed.
///     // std::fs::remove_dir_all(temp_dir_path)?;
/// }
/// # Ok(())
/// # }
/// ```
pub fn download_directory_via_api(
    url_parts: &ParsedGitUrl,
    branch_override: &Option<String>,
) -> Result<PathBuf> {
    // 1. Setup
    let temp_dir = TempDirBuilder::new().prefix("dircat-git-api-").tempdir()?;
    let client = build_reqwest_client()?;
    let (owner, repo) = parse_clone_url(&url_parts.clone_url)?;

    // 2. Resolve branch
    let branch_to_use = if let Some(branch_override) = branch_override {
        // Always prioritize the branch specified on the command line.
        log::debug!("Using branch from override: {}", branch_override);
        branch_override.clone()
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
    use anyhow::Context;
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
