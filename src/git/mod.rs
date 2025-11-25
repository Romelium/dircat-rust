// src/git/mod.rs
//! Handles cloning, caching, and updating of git repositories.
//!
//! This module provides functionality to:
//! - Clone remote git repositories into a local cache using `git2`.
//! - Update cached repositories on subsequent runs.
//! - Parse GitHub folder URLs and download their contents via the GitHub API using `reqwest`.
//! - Provide authentication callbacks for SSH-based cloning.

// Declare the sub-modules.
mod api;
mod clone;
mod ops;
mod url;

// Re-export the public-facing API.
/// Downloads a directory's contents from the GitHub API.
///
/// # Examples
///
/// ```no_run
/// use dircat::git::{parse_github_folder_url, download_directory_via_api};
///
/// # fn main() -> anyhow::Result<()> {
/// let url = "https://github.com/rust-lang/cargo/tree/master/src/cargo";
/// let parsed_url = parse_github_folder_url(url).unwrap();
/// let temp_dir_path = download_directory_via_api(&parsed_url, &None, None)?;
/// println!("Downloaded to: {}", temp_dir_path.display());
/// // Remember to clean up the temp directory if needed.
/// # Ok(())
/// # }
/// ```
pub use api::download_directory_via_api;
/// Clones or updates a git repository into a local cache directory.
///
/// # Examples
///
/// ```no_run
/// use dircat::git::get_repo;
/// use std::path::Path;
/// # fn main() -> anyhow::Result<()> {
/// let repo_path = get_repo("https://github.com/rust-lang/cargo.git", &None, None, Path::new("/tmp/dircat-cache"), None, None)?;
/// println!("Repo is at: {}", repo_path.display());
/// # Ok(())
/// # }
/// ```
pub use clone::{get_repo, get_repo_cache_path};
/// Functions and types for parsing git and GitHub URLs.
pub use ops::update_repo;
pub use url::{
    is_git_url, parse_clone_url, parse_github_folder_url, parse_github_folder_url_with_hint,
    ParsedGitUrl,
};
