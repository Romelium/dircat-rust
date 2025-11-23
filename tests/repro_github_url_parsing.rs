mod common;

#[cfg(feature = "git")]
use dircat::git::parse_github_folder_url_with_hint;

#[test]
#[cfg(feature = "git")]
fn test_parse_github_url_with_slash_in_branch() {
    // A valid GitHub URL pointing to a file on a branch named "feature/new-ui".
    // GitHub URLs use /tree/<branch>/<path> structure.
    let url = "https://github.com/user/repo/tree/feature/new-ui/src/main.rs";

    let parsed = parse_github_folder_url_with_hint(url, Some("feature/new-ui"));

    assert!(
        parsed.is_some(),
        "Failed to parse GitHub URL with slash in branch name"
    );

    let p = parsed.unwrap();
    assert_eq!(
        p.branch, "feature/new-ui",
        "Branch name was not captured correctly"
    );
    assert_eq!(
        p.subdirectory, "src/main.rs",
        "Subdirectory was not captured correctly"
    );
}
