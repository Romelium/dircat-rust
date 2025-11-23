// src/config/path_resolve.rs

#[cfg(not(feature = "git"))]
use crate::errors::ConfigError;
#[cfg(feature = "git")]
use crate::errors::GitError;
use crate::errors::{Error, Result};
#[cfg(feature = "git")]
use crate::git;
use crate::progress::ProgressReporter;
use anyhow::{Context, Result as AnyhowResult};
#[cfg(feature = "git")]
use git2::Repository;
#[cfg(feature = "git")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Holds the results of resolving the user's input path.
///
/// This struct contains the canonicalized, absolute path to the resource to be
/// processed, along with other relevant metadata derived during resolution.
///
/// # Examples
///
/// ```
/// use dircat::config::ResolvedInput;
/// use std::path::PathBuf;
///
/// let resolved = ResolvedInput {
///     path: PathBuf::from("/tmp/dircat-test-cache/abcdef123..."),
///     display: "https://github.com/user/repo.git".to_string(),
///     is_file: false,
///     #[cfg(feature = "git")]
///     cache_path: PathBuf::from("/tmp/dircat-test-cache"),
/// };
///
/// assert_eq!(resolved.display, "https://github.com/user/repo.git");
/// ```
#[derive(Debug, Clone)]
pub struct ResolvedInput {
    /// The resolved, absolute path to the directory or file to be processed.
    pub path: PathBuf,
    /// The original input path string provided by the user, for display purposes.
    pub display: String,
    /// A flag indicating if the resolved path points to a single file.
    pub is_file: bool,
    #[cfg(feature = "git")]
    /// The resolved, absolute path to the directory used for caching git repositories.
    pub cache_path: PathBuf,
}

impl ResolvedInput {
    /// Creates a default `ResolvedInput` for testing purposes.
    #[doc(hidden)]
    pub fn default_for_test() -> Self {
        Self {
            path: PathBuf::from("."),
            display: ".".to_string(),
            is_file: false,
            #[cfg(feature = "git")]
            cache_path: PathBuf::from("/tmp/dircat-test-cache"),
        }
    }
}

#[cfg(feature = "git")]
/// Resolves the input path, handling local paths and git URLs.
///
/// This is the primary entry point for the I/O-heavy part of configuration. It will:
/// - Determine the correct git cache directory.
/// - Check if the input is a git URL and clone/update it if necessary.
/// - Check if the input is a GitHub folder URL and download it via the API.
/// - Resolve a local path to its absolute, canonicalized form.
///
/// # Arguments
/// * `input_path_str` - The path or URL string from the user.
/// * `git_branch` - The specific git branch/tag to use for git URLs.
/// * `git_depth` - The depth for a shallow git clone.
/// * `git_cache_path_str` - The user-specified path for the git cache.
/// * `progress` - An optional progress reporter for long operations like cloning.
///
/// # Returns
/// A `Result` containing a `ResolvedInput` struct on success.
///
/// # Errors
/// Returns an `Error` if path resolution fails, git operations fail, or the
/// input is a git URL but the `git` feature is disabled.
///
/// # Examples
///
/// **Conceptual Example:**
///
/// ```no_run
/// use dircat::config::resolve_input;
/// # use dircat::errors::Result;
/// # fn main() -> Result<()> {
/// // Resolve a local path string into a structured, absolute path.
/// let resolved = resolve_input("./src", &None, None, &None, None)?;
///
/// assert!(resolved.path.is_absolute());
/// assert_eq!(resolved.display, "./src");
///
/// // Resolve a git URL (this would perform a clone/update).
/// let resolved_git = resolve_input("https://github.com/user/repo.git", &None, None, &None, None)?;
/// assert!(resolved_git.path.to_str().unwrap().contains("dircat/repos"));
/// # Ok(())
/// # }
/// ```
///
/// **Filesystem Example:**
///
/// ```
/// use dircat::config::resolve_input;
/// use dircat::progress::ProgressReporter;
/// use std::sync::Arc;
/// use std::fs;
/// use tempfile::tempdir;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a temporary directory and file for the example.
/// let temp = tempdir()?;
/// let file_path = temp.path().join("test.txt");
/// fs::write(&file_path, "content")?;
///
/// // The progress reporter is optional and can be None.
/// let progress: Option<Arc<dyn ProgressReporter>> = None;
///
/// // --- Example 1: Resolve a local file path ---
/// // The git-related arguments are None for local paths.
/// let resolved_file = resolve_input(
///     file_path.to_str().unwrap(),
///     &None, // git_branch
///     None,  // git_depth
///     &None, // git_cache_path
///     progress.clone(),
/// )?;
///
/// assert!(resolved_file.path.is_absolute());
/// assert!(resolved_file.is_file);
///
/// // --- Example 2: Resolve a local directory path ---
/// let resolved_dir = resolve_input(temp.path().to_str().unwrap(), &None, None, &None, progress)?;
/// assert!(!resolved_dir.is_file);
/// # Ok(())
/// # }
/// ```
pub fn resolve_input(
    input_path_str: &str,
    git_branch: &Option<String>,
    git_depth: Option<u32>,
    git_cache_path_str: &Option<String>,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<ResolvedInput> {
    let cache_path = determine_cache_dir(git_cache_path_str.as_deref()).map_err(Error::from)?;

    let absolute_path = if let Some(parsed_url) =
        git::parse_github_folder_url_with_hint(input_path_str, git_branch.as_deref())
    {
        log::debug!("Input detected as GitHub folder URL: {:?}", parsed_url);
        handle_github_folder_url(parsed_url, git_branch, &cache_path, progress)?
    } else if git::is_git_url(input_path_str) {
        git::get_repo(input_path_str, git_branch, git_depth, &cache_path, progress)
            .map_err(Error::from)?
    } else {
        resolve_local_input_path(input_path_str).map_err(Error::from)?
    };

    Ok(ResolvedInput {
        is_file: absolute_path.is_file(),
        path: absolute_path,
        display: input_path_str.to_string(),
        #[cfg(feature = "git")]
        cache_path,
    })
}

/// Logic for handling a parsed GitHub folder URL, including API download and fallback to clone.
#[cfg(feature = "git")]
fn handle_github_folder_url(
    parsed_url: git::ParsedGitUrl,
    cli_branch: &Option<String>,
    cache_path: &Path,
    progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<PathBuf> {
    let repo_cache_path = git::get_repo_cache_path(cache_path, &parsed_url.clone_url);

    // Optimization: If a full clone of the repository already exists in the cache,
    // update and use it directly instead of hitting the GitHub API.
    if let Ok(repo) = Repository::open(&repo_cache_path) {
        log::info!(
            "Found cached repository at '{}'. Updating and using it instead of GitHub API.",
            repo_cache_path.display()
        );
        // Update the repo to the desired branch/ref.
        git::update_repo(&repo, cli_branch, None, progress)?;

        let path = repo_cache_path.join(&parsed_url.subdirectory);
        if !path.exists() {
            return Err(Error::Git(GitError::SubdirectoryNotFound {
                path: parsed_url.subdirectory,
                repo: parsed_url.clone_url,
            }));
        }
        return Ok(path);
    }

    // If no valid cache entry, proceed with API download and its own fallback logic.
    match git::download_directory_via_api(&parsed_url, cli_branch) {
        Ok(temp_dir_root) => {
            log::debug!("Successfully downloaded from GitHub API.");
            let path = temp_dir_root.join(&parsed_url.subdirectory);
            if !path.exists() {
                return Err(Error::Git(GitError::SubdirectoryNotFound {
                    path: parsed_url.subdirectory,
                    repo: parsed_url.clone_url,
                }));
            }
            Ok(path)
        }
        Err(e) => {
            let is_rate_limit_error = e
                .downcast_ref::<reqwest::Error>()
                .is_some_and(|re| re.status() == Some(reqwest::StatusCode::FORBIDDEN));

            if is_rate_limit_error {
                log::warn!(
                        "GitHub API request failed (likely rate-limited). Falling back to a full git clone of '{}'.",
                        parsed_url.clone_url
                    );
                log::warn!(
                    "To avoid this, set a GITHUB_TOKEN environment variable with 'repo' scope."
                );

                let cloned_repo_root = git::get_repo(
                    &parsed_url.clone_url,
                    cli_branch,
                    None, // No shallow clone for fallback
                    cache_path,
                    progress,
                )
                .map_err(Error::from)?;
                let path = cloned_repo_root.join(&parsed_url.subdirectory);
                if !path.exists() {
                    return Err(Error::Git(GitError::SubdirectoryNotFound {
                        path: parsed_url.subdirectory,
                        repo: parsed_url.clone_url,
                    }));
                }
                Ok(path)
            } else {
                if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                    if reqwest_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                        return Err(Error::Git(GitError::SubdirectoryNotFound {
                            path: parsed_url.subdirectory,
                            repo: parsed_url.clone_url,
                        }));
                    }
                }
                Err(Error::Git(GitError::ApiDownloadFailed {
                    url: parsed_url.clone_url,
                    source: e,
                }))
            }
        }
    }
}

/// Resolves the input path string to an absolute, canonicalized PathBuf.
fn resolve_local_input_path(input_path_str: &str) -> AnyhowResult<PathBuf> {
    let input_path = PathBuf::from(input_path_str);
    input_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve input path: '{}'", input_path_str))
}

#[cfg(not(feature = "git"))]
/// Resolves the input path, handling only local paths.
///
/// This is the non-git version of the function. It will return an error if the
/// input path appears to be a git URL.
///
/// # Arguments
/// * `input_path_str` - The path string from the user.
/// * `_git_branch`, `_git_depth`, `_git_cache_path_str`, `_progress` - Unused arguments for API compatibility.
///
/// # Returns
/// A `Result` containing a `ResolvedInput` struct on success.
///
/// # Errors
/// Returns an `Error` if path resolution fails or if the input appears to be a URL.
///
/// # Examples
///
/// ```
/// use dircat::config::resolve_input;
/// use dircat::progress::ProgressReporter;
/// use std::sync::Arc;
/// use std::fs;
/// use tempfile::tempdir;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a temporary directory and file for the example.
/// let temp = tempdir()?;
/// let file_path = temp.path().join("test.txt");
/// fs::write(&file_path, "content")?;
///
/// // The progress reporter is optional and can be None.
/// let progress: Option<Arc<dyn ProgressReporter>> = None;
///
/// // --- Example 1: Resolve a local file path ---
/// // The git-related arguments are ignored in non-git builds.
/// let resolved_file = resolve_input(
///     file_path.to_str().unwrap(),
///     &None, // git_branch
///     None,  // git_depth
///     &None, // git_cache_path
///     progress.clone(),
/// )?;
///
/// assert!(resolved_file.path.is_absolute());
/// assert!(resolved_file.is_file);
///
/// // --- Example 2: Resolve a local directory path ---
/// let resolved_dir = resolve_input(temp.path().to_str().unwrap(), &None, None, &None, progress)?;
/// assert!(!resolved_dir.is_file);
/// # Ok(())
/// # }
/// ```
pub fn resolve_input(
    input_path_str: &str,
    _git_branch: &Option<String>,
    _git_depth: Option<u32>,
    _git_cache_path_str: &Option<String>,
    _progress: Option<Arc<dyn ProgressReporter>>,
) -> Result<ResolvedInput> {
    // Check for likely URL patterns and return a helpful error if found.
    if input_path_str.starts_with("https://")
        || input_path_str.starts_with("http://")
        || input_path_str.starts_with("git@")
    {
        return Err(Error::Config(ConfigError::InvalidValue {
            option: "input_path".to_string(),
            reason:
                "Git URLs are not supported because dircat was compiled without the 'git' feature."
                    .to_string(),
        }));
    }

    let absolute_path = resolve_local_input_path(input_path_str).map_err(Error::from)?;

    Ok(ResolvedInput {
        is_file: absolute_path.is_file(),
        path: absolute_path,
        display: input_path_str.to_string(),
    })
}

/// Determines the absolute path for the git cache directory.
///
/// The cache directory is determined in the following order of precedence:
/// 1. The `DIRCAT_TEST_CACHE_DIR` environment variable (for testing purposes).
/// 2. The path provided via the `--git-cache-path` command-line argument.
/// 3. The platform-specific default cache directory (e.g., `~/.cache/dircat/repos` on Linux).
///
/// If the chosen directory does not exist, it will be created.
///
/// # Arguments
/// * `cli_path` - An optional path string from the command-line arguments.
///
/// # Returns
/// A `Result` containing the absolute `PathBuf` to the cache directory.
///
/// # Errors
/// Returns an error if the directory cannot be determined or created.
#[cfg(feature = "git")]
/// # Examples
///
/// ```
/// use dircat::config::determine_cache_dir;
/// use tempfile::tempdir;
/// # use std::env;
///
/// # fn main() -> anyhow::Result<()> {
/// // --- Case 1: Using a CLI path ---
/// let temp = tempdir()?;
/// let cli_path_str = temp.path().to_str().unwrap();
/// let cache_path = determine_cache_dir(Some(cli_path_str))?;
/// assert_eq!(cache_path, temp.path().canonicalize()?);
///
/// // --- Case 2: Using the default path (cannot be easily tested for exact path) ---
/// // let default_path = determine_cache_dir(None)?;
/// // assert!(default_path.ends_with("dircat/repos"));
///
/// // --- Case 3: Using an environment variable for testing ---
/// let test_cache_dir = tempdir()?;
/// env::set_var("DIRCAT_TEST_CACHE_DIR", test_cache_dir.path());
/// let env_cache_path = determine_cache_dir(None)?;
/// assert_eq!(env_cache_path, test_cache_dir.path());
/// env::remove_var("DIRCAT_TEST_CACHE_DIR");
/// # Ok(())
/// # }
/// ```
pub fn determine_cache_dir(cli_path: Option<&str>) -> AnyhowResult<PathBuf> {
    if let Ok(cache_override) = std::env::var("DIRCAT_TEST_CACHE_DIR") {
        let path = PathBuf::from(cache_override);
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        return Ok(path);
    }

    if let Some(path_str) = cli_path {
        let path = PathBuf::from(path_str);
        // Ensure the directory exists and is absolute.
        if !path.exists() {
            std::fs::create_dir_all(&path).with_context(|| {
                format!(
                    "Failed to create specified git cache directory at '{}'",
                    path.display()
                )
            })?;
        }
        return path.canonicalize().with_context(|| {
            format!(
                "Failed to resolve specified git cache path: '{}'",
                path.display()
            )
        });
    }

    // Fallback to default project cache directory.
    let proj_dirs = directories::ProjectDirs::from("com", "romelium", "dircat")
        .context("Could not determine project cache directory")?;
    let cache_dir = proj_dirs.cache_dir().join("repos");
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir).with_context(|| {
            format!(
                "Failed to create default git cache directory at '{}'",
                cache_dir.display()
            )
        })?;
    }
    Ok(cache_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Error;
    #[cfg(feature = "git")]
    use crate::errors::GitError;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_existing_dir() -> Result<()> {
        let temp = tempdir()?;
        let path_str = temp.path().to_str().unwrap();
        let resolved = resolve_local_input_path(path_str)?;
        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        assert!(resolved.is_dir());
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_resolve_existing_file() -> Result<()> {
        let temp = tempdir()?;
        let file_path = temp.path().join("test.txt");
        fs::write(&file_path, "content")?;
        let path_str = file_path.to_str().unwrap();
        let resolved = resolve_local_input_path(path_str)?;
        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        assert!(resolved.is_file());
        temp.close()?;
        Ok(())
    }

    #[test]
    fn test_resolve_non_existent_path() {
        let result = resolve_local_input_path("non_existent_path_for_testing_dircat");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to resolve input path"));
    }

    #[test]
    fn test_resolve_relative_path() -> Result<()> {
        // Create a file in the current dir to resolve relatively
        let filename = "relative_test_file_dircat.txt";
        fs::write(filename, "relative")?;
        let resolved = resolve_local_input_path(filename)?;
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with(filename));
        fs::remove_file(filename)?; // Clean up
        Ok(())
    }

    // --- Tests for the public resolve_input function ---

    #[test]
    fn test_resolve_input_local_path_success() -> Result<()> {
        let temp = tempdir()?;
        let path_str = temp.path().to_str().unwrap();

        let resolved = resolve_input(path_str, &None, None, &None, None)?;

        assert_eq!(resolved.path, temp.path().canonicalize()?);
        assert_eq!(resolved.display, path_str);
        assert!(!resolved.is_file);
        #[cfg(feature = "git")]
        assert!(resolved.cache_path.exists());

        Ok(())
    }

    #[test]
    fn test_resolve_input_local_path_failure() {
        let result = resolve_input(
            "non_existent_path_for_dircat_testing",
            &None,
            None,
            &None,
            None,
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Generic(_)));
    }

    /// Helper to create a local bare git repo to act as a "remote" for offline tests.
    #[cfg(feature = "git")]
    fn setup_local_git_repo() -> Result<(tempfile::TempDir, String)> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path();
        let repo = git2::Repository::init_bare(repo_path)?;

        // Create a commit
        let signature = git2::Signature::now("Test", "test@example.com")?;
        let tree_id = {
            let mut index = repo.index()?;
            let oid = repo.blob("content".as_bytes())?;
            let entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: 0o100644,
                uid: 0,
                gid: 0,
                file_size: 7,
                id: oid,
                flags: 0,
                flags_extended: 0,
                path: b"file.txt".to_vec(),
            };
            index.add(&entry)?;
            index.write_tree()?
        };
        let tree = repo.find_tree(tree_id)?;
        repo.commit(Some("HEAD"), &signature, &signature, "Initial", &tree, &[])?;

        #[cfg(windows)]
        let url = format!("file:///{}", repo_path.to_str().unwrap().replace('\\', "/"));
        #[cfg(not(windows))]
        let url = format!("file://{}", repo_path.to_str().unwrap());

        Ok((temp_dir, url))
    }

    #[test]
    #[cfg(feature = "git")]
    fn test_resolve_input_git_url_local_success() -> Result<()> {
        let (_remote_dir, remote_url) = setup_local_git_repo()?;
        let cache_dir = tempdir()?;

        // Set the environment variable for this test to ensure isolation.
        // This is the primary mechanism for controlling the cache path in tests.
        std::env::set_var("DIRCAT_TEST_CACHE_DIR", cache_dir.path());

        let resolved = resolve_input(
            &remote_url,
            &None,
            None,
            &None, // Pass None for the CLI arg, so it uses the env var
            None,
        )?;

        assert!(resolved.path.starts_with(cache_dir.path()));
        assert!(resolved.path.join("file.txt").exists());
        assert_eq!(
            fs::read_to_string(resolved.path.join("file.txt"))?,
            "content"
        );
        assert_eq!(resolved.display, remote_url);
        assert!(!resolved.is_file);

        // Clean up the environment variable after the test.
        std::env::remove_var("DIRCAT_TEST_CACHE_DIR");
        Ok(())
    }

    #[test]
    #[cfg(feature = "git")]
    #[ignore = "requires network access and is slow"]
    fn test_resolve_input_git_url_remote_failure() {
        let invalid_url = "https://github.com/user/this-repo-will-never-exist-probably.git";
        let result = resolve_input(invalid_url, &None, None, &None, None);
        assert!(matches!(
            result,
            Err(Error::Git(GitError::CloneFailed { .. }))
        ));
    }

    #[test]
    #[cfg(feature = "git")]
    #[ignore = "requires network access and is slow"]
    fn test_resolve_input_github_folder_url_success() -> Result<()> {
        let folder_url = "https://github.com/git-fixtures/basic/tree/master/go";
        let resolved = resolve_input(folder_url, &None, None, &None, None)?;

        assert!(resolved.path.is_dir());
        assert!(resolved.path.join("example.go").exists());
        assert_eq!(resolved.display, folder_url);

        Ok(())
    }

    #[test]
    #[cfg(feature = "git")]
    #[ignore = "requires network access and is slow"]
    fn test_resolve_input_git_clone_error_returns_structured_error() {
        // Use a URL that is syntactically valid but points to a non-existent repo
        let invalid_git_url = "https://github.com/romelium/this-repo-does-not-exist.git";
        let result = resolve_input(invalid_git_url, &None, None, &None, None);

        assert!(matches!(
            result,
            Err(Error::Git(GitError::CloneFailed { .. }))
        ));
        let err_msg = result.unwrap_err().to_string();
        // Check for a substring that indicates a clone failure
        assert!(err_msg.contains("Failed to clone repository"));
    }
}
