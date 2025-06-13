// src/git.rs

use crate::config::Config;
use anyhow::{Context, Result};
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks};
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

/// Clones a git repository from a URL into a new temporary directory.
/// Returns the TempDir object (for RAII cleanup) and the path to the cloned repo.
pub fn clone_repo(url: &str, config: &Config) -> Result<(TempDir, PathBuf)> {
    let temp_dir = tempdir().context("Failed to create temporary directory for git clone")?;
    let repo_path = temp_dir.path().to_path_buf();

    log::info!(
        "Cloning git repository from '{}' into '{}'...",
        url,
        repo_path.display()
    );

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

    // --- Setup Fetch Options ---
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    if let Some(depth) = config.git_depth {
        fetch_options.depth(depth as i32);
        log::debug!("Set shallow clone depth to: {}", depth);
    }

    // --- Build and run the clone operation ---
    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);
    if let Some(branch) = &config.git_branch {
        repo_builder.branch(branch);
        log::debug!("Set custom branch to: {}", branch);
    }

    repo_builder
        .clone(url, &repo_path)
        .with_context(|| format!("Failed to clone git repository from URL: {}", url))?;

    progress_bar.finish_with_message("Clone complete.");

    log::info!("Successfully cloned repository.");
    Ok((temp_dir, repo_path))
}

/// Checks if a given string is a likely git repository URL.
pub fn is_git_url(path_str: &str) -> bool {
    path_str.starts_with("https://")
        || path_str.starts_with("http://")
        || path_str.starts_with("git@")
        || path_str.starts_with("file://")
}
