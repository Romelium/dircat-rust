//! Handles parsing of git and GitHub URLs.

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;

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

/// Parses the owner and repository name from a GitHub clone URL.
///
/// This function handles common GitHub URL formats, including `https://...` and `git@...`,
/// and with or without a `.git` suffix.
///
/// # Arguments
/// * `clone_url` - The GitHub repository URL to parse.
///
/// # Returns
/// A `Result` containing a tuple of `(owner, repo)` strings on success.
///
/// # Examples
/// ```
/// # use dircat::git::parse_clone_url;
/// let (owner, repo) = parse_clone_url("https://github.com/romelium/dircat-rust.git").unwrap();
/// assert_eq!(owner, "romelium");
/// assert_eq!(repo, "dircat-rust");
///
/// let (owner, repo) = parse_clone_url("git@github.com:rust-lang/cargo.git").unwrap();
/// assert_eq!(owner, "rust-lang");
/// assert_eq!(repo, "cargo");
/// ```
pub fn parse_clone_url(clone_url: &str) -> Result<(String, String)> {
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
