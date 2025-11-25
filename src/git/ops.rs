// src/git/ops.rs
//! Low-level git operations using `git2`.

use crate::errors::GitError;
use crate::progress::ProgressReporter;
use anyhow::{anyhow, Context, Result};
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository, ResetType};
use log::{debug, info, warn};
use std::sync::Arc;

/// Sets up remote callbacks for authentication and progress reporting.
pub(super) fn create_remote_callbacks(
    progress: Option<Arc<dyn ProgressReporter>>,
) -> RemoteCallbacks<'static> {
    // --- Setup Callbacks for Authentication and Progress ---
    let mut callbacks = RemoteCallbacks::new();

    // Authentication: Try SSH agent first, then default key paths.
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        let username = username_from_url.unwrap_or("git");
        debug!("Attempting SSH authentication for user: {}", username);

        // Try to authenticate with an SSH agent
        if let Ok(cred) = Cred::ssh_key_from_agent(username) {
            debug!("Authenticated via SSH agent");
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
            debug!("Authenticated via default SSH key path");
            return Ok(cred);
        }

        warn!("SSH authentication failed: No agent or default keys found.");
        Err(git2::Error::from_str(
            "Authentication failed: could not connect with SSH agent or default keys",
        ))
    });

    if let Some(p) = progress {
        callbacks.transfer_progress(move |stats| {
            // SECURITY: Disk Exhaustion Protection
            // Hard limit of 1GB for transfer if not otherwise specified to prevent zip bombs.
            // In a full refactor, pass max_repo_size through the config.
            if stats.received_bytes() > 1024 * 1024 * 1024 {
                return false; // Abort transfer
            }

            if stats.received_objects() == stats.total_objects() {
                p.set_length(stats.total_deltas() as u64);
                p.set_position(stats.indexed_deltas() as u64);
                p.set_message("Resolving deltas...".to_string());
            } else if stats.total_objects() > 0 {
                p.set_length(stats.total_objects() as u64);
                p.set_position(stats.received_objects() as u64);
                p.set_message("Receiving objects...".to_string());
            }
            true
        });
    }

    callbacks
}

pub(super) fn create_fetch_options(
    depth: Option<u32>,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> FetchOptions<'static> {
    // --- Setup Fetch Options ---
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(create_remote_callbacks(progress));
    // Enable pruning to remove remote-tracking branches that no longer exist on the remote.
    fetch_options.prune(git2::FetchPrune::On);
    // SECURITY: Disable redirects to prevent protocol downgrades or SSRF via redirection
    // to file:// or internal git servers.
    fetch_options.follow_redirects(git2::RemoteRedirect::None);
    // Download all tags from the remote. This is important for checking out tags.
    fetch_options.download_tags(git2::AutotagOption::All);
    if let Some(depth) = depth {
        fetch_options.depth(depth as i32);
        debug!("Set shallow clone depth to: {}", depth);
    }

    fetch_options
}

/// Finds the target commit on the remote that the repository should be updated to.
///
/// It determines the target commit by:
/// 1. Using the branch specified by `--git-branch`.
/// 2. If no branch is specified, resolving the remote's `HEAD` to find the default branch.
pub(super) fn find_remote_commit<'a>(
    repo: &'a Repository,
    branch: &Option<String>,
) -> Result<git2::Commit<'a>, GitError> {
    if let Some(ref_name) = branch {
        debug!("Using user-specified ref: {}", ref_name);

        // 1. Try to resolve as a remote branch.
        let branch_ref_name = format!("refs/remotes/origin/{}", ref_name);
        if let Ok(reference) = repo.find_reference(&branch_ref_name) {
            debug!("Resolved '{}' as a remote branch.", ref_name);
            return repo
                .find_commit(
                    reference
                        .target()
                        .context("Remote branch reference has no target commit")?,
                )
                .context("Failed to find commit for branch reference")
                .map_err(GitError::Generic);
        }

        // 2. Try to resolve as a tag.
        let tag_ref_name = format!("refs/tags/{}", ref_name);
        if let Ok(reference) = repo.find_reference(&tag_ref_name) {
            debug!("Resolved '{}' as a tag.", ref_name);
            // A tag can be lightweight (points directly to a commit) or annotated
            // (points to a tag object, which then points to a commit).
            // `peel_to_commit` handles both cases.
            let object = reference
                .peel(git2::ObjectType::Commit)
                .map_err(|e| GitError::Generic(anyhow!(e)))?;
            return object.into_commit().map_err(|_| {
                GitError::Generic(anyhow!("Tag '{}' does not point to a commit", ref_name))
            });
        }

        // 3. If both fail, return a comprehensive error.
        return Err(GitError::RefNotFound {
            name: ref_name.clone(),
        });
    }

    // User did not specify a branch, so find the remote's default by resolving its HEAD.
    debug!("Resolving remote's default branch via origin/HEAD");
    let remote_head = repo.find_reference("refs/remotes/origin/HEAD")
        .context("Could not find remote's HEAD. The repository might not have a default branch set, or it may be empty. Please specify a branch with --git-branch.")
        .map_err(|e| GitError::DefaultBranchResolution(e.to_string()))?;
    let remote_branch_ref_name = remote_head
        .symbolic_target()
        .context("Remote HEAD is not a symbolic reference; cannot determine default branch.")
        .map_err(|e| GitError::DefaultBranchResolution(e.to_string()))?
        .to_string();

    debug!("Targeting remote reference: {}", remote_branch_ref_name);
    let fetch_head = repo
        .find_reference(&remote_branch_ref_name)
        .with_context(|| {
            format!(
                "Could not find remote branch reference '{}' after fetch. Does this branch exist on the remote?",
                remote_branch_ref_name
            )
        })
        .map_err(GitError::Generic)?;
    repo.find_commit(
        fetch_head
            .target()
            .context("Remote branch reference has no target commit")?,
    )
    .context("Failed to find commit for default branch reference")
    .map_err(GitError::Generic)
}
/// Ensures a local repository is up-to-date with its 'origin' remote.
///
/// This function performs the following actions:
/// 1.  Fetches the latest changes, tags, and branches from the 'origin' remote.
/// 2.  Prunes any remote-tracking branches that no longer exist on the remote.
/// 3.  Determines the target commit on the remote (either a specific branch/tag or the remote's default branch).
/// 4.  Performs a hard reset of the local repository to match the target commit, discarding any local changes.
///
/// This is primarily used to update a cached repository to the desired state.
///
/// # Arguments
/// * `repo` - The `git2::Repository` instance to update.
/// * `branch` - An optional specific branch or tag name to check out. If `None`, the remote's default branch is used.
/// * `depth` - An optional depth for a shallow fetch.
/// * `progress` - An optional progress reporter for the fetch operation.
///
/// # Errors
/// Returns a `GitError` if the remote cannot be found, the fetch fails, or the specified ref cannot be resolved.
///
/// # Examples
///
/// ```no_run
/// use dircat::git::update_repo;
/// use git2::Repository;
/// use std::path::Path;
/// # use anyhow::Result;
///
/// # fn main() -> Result<()> {
/// // Assume `repo_path` points to a local clone of a git repository.
/// let repo_path = Path::new("/tmp/dircat-cache/some-repo-hash");
///
/// // In a real application, you would open an existing repository.
/// if repo_path.exists() {
///     let repo = Repository::open(repo_path)?;
///
///     // Update the repo to the latest commit on the remote's default branch.
///     update_repo(&repo, &None, None, None)?;
///
///     // Update the repo to a specific tag named "v1.2.0".
///     update_repo(&repo, &Some("v1.2.0".to_string()), None, None)?;
/// }
///
/// # Ok(())
/// # }
/// ```
pub fn update_repo(
    repo: &Repository,
    branch: &Option<String>,
    depth: Option<u32>,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<(), GitError> {
    info!("Updating cached repository...");
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| GitError::Generic(anyhow!(e)))?;
    let mut fetch_options = create_fetch_options(depth, progress);

    // Fetch updates from the remote
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .map_err(|e| GitError::FetchFailed {
            remote: "origin".to_string(),
            source: e,
        })?;

    // Find the specific commit we need to reset to.
    let target_commit = find_remote_commit(repo, branch)?;
    // Detach HEAD before resetting to avoid issues with checked-out branches.
    repo.set_head_detached(target_commit.id())
        .context("Failed to detach HEAD in cached repository")
        .map_err(|e| GitError::UpdateFailed(e.to_string()))?;

    // Reset the local repository to match the fetched commit
    repo.reset(
        target_commit.as_object(),
        ResetType::Hard,
        None, // No checkout builder needed for hard reset
    )
    .context("Failed to perform hard reset on cached repository")
    .map_err(|e| GitError::UpdateFailed(e.to_string()))?;

    info!("Cached repository updated successfully.");
    Ok(())
}
