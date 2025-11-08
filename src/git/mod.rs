// src/git/mod.rs
//! Handles cloning, caching, and updating of git repositories provided as input.
//!
//! This module provides functionality to:
//! - Clone remote git repositories into a local cache using `git2`.
//! - Update cached repositories on subsequent runs.
//! - Parse GitHub folder URLs and download their contents via the GitHub API using `reqwest`.
//! - Provide authentication callbacks for SSH.

// Declare the sub-modules.
mod api;
mod clone;
mod url;

// Re-export the public-facing API.
pub use api::download_directory_via_api;
pub use clone::get_repo;
pub use url::{is_git_url, parse_clone_url, parse_github_folder_url, ParsedGitUrl};
