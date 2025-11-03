// src/config/builder.rs

//! Builds the `Config` struct from command-line arguments or other sources.
//!
//! This module provides the `ConfigBuilder`, which is the primary entry point for
//! constructing the application's configuration. It handles validation, path
//! resolution, and git repository cloning/downloading.

use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    path_resolve::resolve_input_path,
    Config, OutputDestination,
};
use crate::cli::Cli;
use crate::errors::{Error, Result};
use crate::git;
use crate::processing::filters::{ContentFilter, RemoveCommentsFilter, RemoveEmptyLinesFilter};
use crate::progress::ProgressReporter;
use anyhow::{Context, Result as AnyhowResult};
use directories::ProjectDirs;
use std::path::PathBuf;
use std::sync::Arc;

/// A builder for creating a `Config` instance from command-line arguments or programmatically.
///
/// This builder handles argument validation, path resolution, and git repository cloning/downloading.
///
/// # Examples
///
/// ```
/// use dircat::config::ConfigBuilder;
///
/// // Programmatic construction
/// let config = ConfigBuilder::new()
///     .input_path(".")
///     .extensions(vec!["rs".to_string()])
///     .summary(true)
///     .build(None) // Pass None for no progress reporter
///     .unwrap();
///
/// assert!(config.summary);
/// assert_eq!(config.extensions, Some(vec!["rs".to_string()]));
/// ```
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    // --- Input ---
    input_path: Option<String>,
    // --- Git Options ---
    git_branch: Option<String>,
    git_depth: Option<u32>,
    git_cache_path: Option<String>,
    // --- Filtering Options ---
    max_size: Option<String>,
    no_recursive: Option<bool>,
    extensions: Option<Vec<String>>,
    exclude_extensions: Option<Vec<String>>,
    exclude_path_regex: Option<Vec<String>>,
    ignore_patterns: Option<Vec<String>>,
    path_regex: Option<Vec<String>>,
    filename_regex: Option<Vec<String>>,
    no_gitignore: Option<bool>,
    include_binary: Option<bool>,
    no_lockfiles: Option<bool>,
    // --- Content Processing Options ---
    remove_comments: Option<bool>,
    remove_empty_lines: Option<bool>,
    // --- Output Formatting Options ---
    filename_only: Option<bool>,
    line_numbers: Option<bool>,
    backticks: Option<bool>,
    ticks: Option<u8>,
    // --- Output Destination & Summary ---
    output_file: Option<String>,
    paste: Option<bool>,
    summary: Option<bool>,
    counts: Option<bool>,
    // --- Processing Order ---
    process_last: Option<Vec<String>>,
    only_last: Option<bool>,
    only: Option<Vec<String>>,
    // --- Execution Control ---
    dry_run: Option<bool>,
}

impl ConfigBuilder {
    /// Creates a new `ConfigBuilder` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `ConfigBuilder` populated from the parsed command-line interface arguments.
    pub fn from_cli(cli: Cli) -> Self {
        Self {
            input_path: Some(cli.input_path),
            git_branch: cli.git_branch,
            git_depth: cli.git_depth,
            git_cache_path: cli.git_cache_path,
            max_size: cli.max_size,
            no_recursive: Some(cli.no_recursive),
            extensions: cli.extensions,
            exclude_extensions: cli.exclude_extensions,
            exclude_path_regex: cli.exclude_path_regex,
            ignore_patterns: cli.ignore_patterns,
            path_regex: cli.path_regex,
            filename_regex: cli.filename_regex,
            no_gitignore: Some(cli.no_gitignore),
            include_binary: Some(cli.include_binary),
            no_lockfiles: Some(cli.no_lockfiles),
            remove_comments: Some(cli.remove_comments),
            remove_empty_lines: Some(cli.remove_empty_lines),
            filename_only: Some(cli.filename_only),
            line_numbers: Some(cli.line_numbers),
            backticks: Some(cli.backticks),
            ticks: Some(cli.ticks),
            output_file: cli.output_file,
            paste: Some(cli.paste),
            summary: Some(cli.summary),
            counts: Some(cli.counts),
            process_last: cli.process_last,
            only_last: Some(cli.only_last),
            only: cli.only,
            dry_run: Some(cli.dry_run),
        }
    }

    // --- Setter Methods ---
    #[must_use]
    pub fn input_path(mut self, path: impl Into<String>) -> Self {
        self.input_path = Some(path.into());
        self
    }
    #[must_use]
    pub fn git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }
    #[must_use]
    pub fn git_depth(mut self, depth: u32) -> Self {
        self.git_depth = Some(depth);
        self
    }
    #[must_use]
    pub fn git_cache_path(mut self, path: impl Into<String>) -> Self {
        self.git_cache_path = Some(path.into());
        self
    }
    #[must_use]
    pub fn max_size(mut self, size: impl Into<String>) -> Self {
        self.max_size = Some(size.into());
        self
    }
    #[must_use]
    pub fn no_recursive(mut self, no_recurse: bool) -> Self {
        self.no_recursive = Some(no_recurse);
        self
    }
    #[must_use]
    pub fn extensions(mut self, exts: Vec<String>) -> Self {
        self.extensions = Some(exts);
        self
    }
    #[must_use]
    pub fn exclude_extensions(mut self, exts: Vec<String>) -> Self {
        self.exclude_extensions = Some(exts);
        self
    }
    #[must_use]
    pub fn exclude_path_regex(mut self, regexes: Vec<String>) -> Self {
        self.exclude_path_regex = Some(regexes);
        self
    }
    #[must_use]
    pub fn ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = Some(patterns);
        self
    }
    #[must_use]
    pub fn path_regex(mut self, regexes: Vec<String>) -> Self {
        self.path_regex = Some(regexes);
        self
    }
    #[must_use]
    pub fn filename_regex(mut self, regexes: Vec<String>) -> Self {
        self.filename_regex = Some(regexes);
        self
    }
    #[must_use]
    pub fn no_gitignore(mut self, no_gitignore: bool) -> Self {
        self.no_gitignore = Some(no_gitignore);
        self
    }
    #[must_use]
    pub fn include_binary(mut self, include: bool) -> Self {
        self.include_binary = Some(include);
        self
    }
    #[must_use]
    pub fn no_lockfiles(mut self, no_lockfiles: bool) -> Self {
        self.no_lockfiles = Some(no_lockfiles);
        self
    }
    #[must_use]
    pub fn remove_comments(mut self, remove: bool) -> Self {
        self.remove_comments = Some(remove);
        self
    }
    #[must_use]
    pub fn remove_empty_lines(mut self, remove: bool) -> Self {
        self.remove_empty_lines = Some(remove);
        self
    }
    #[must_use]
    pub fn filename_only(mut self, filename_only: bool) -> Self {
        self.filename_only = Some(filename_only);
        self
    }
    #[must_use]
    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = Some(line_numbers);
        self
    }
    #[must_use]
    pub fn backticks(mut self, backticks: bool) -> Self {
        self.backticks = Some(backticks);
        self
    }
    #[must_use]
    pub fn ticks(mut self, count: u8) -> Self {
        self.ticks = Some(count);
        self
    }
    #[must_use]
    pub fn output_file(mut self, path: impl Into<String>) -> Self {
        self.output_file = Some(path.into());
        self
    }
    #[must_use]
    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = Some(paste);
        self
    }
    #[must_use]
    pub fn summary(mut self, summary: bool) -> Self {
        self.summary = Some(summary);
        self
    }
    #[must_use]
    pub fn counts(mut self, counts: bool) -> Self {
        self.counts = Some(counts);
        self
    }
    #[must_use]
    pub fn process_last(mut self, patterns: Vec<String>) -> Self {
        self.process_last = Some(patterns);
        self
    }
    #[must_use]
    pub fn only_last(mut self, only_last: bool) -> Self {
        self.only_last = Some(only_last);
        self
    }
    #[must_use]
    pub fn only(mut self, patterns: Vec<String>) -> Self {
        self.only = Some(patterns);
        self
    }
    #[must_use]
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = Some(dry_run);
        self
    }

    /// Builds the final `Config` struct.
    ///
    /// This method performs all necessary setup and validation:
    /// - Validates option combinations.
    /// - Parses and compiles patterns (regex, extensions).
    /// - Resolves the git cache path.
    /// - Handles the input path, which can be a local path, a git URL (for cloning),
    ///   or a GitHub folder URL (for API download).
    ///
    /// # Errors
    ///
    /// Returns an error if any validation fails, paths cannot be resolved, or git
    /// operations fail.
    pub fn build(self, progress: Option<Arc<dyn ProgressReporter>>) -> Result<Config> {
        // --- Validation ---
        if self.output_file.is_some() && self.paste.unwrap_or(false) {
            return Err(Error::Config(
                "Cannot use --output and --paste simultaneously.".to_string(),
            ));
        }
        if self.ticks.unwrap_or(3) < 3 {
            return Err(Error::Config(
                "The number of ticks must be 3 or greater.".to_string(),
            ));
        }
        if self.only_last.unwrap_or(false) && self.process_last.is_none() {
            return Err(Error::Config(
                "--only-last requires --last to be specified.".to_string(),
            ));
        }
        if self.only.is_some() && (self.process_last.is_some() || self.only_last.unwrap_or(false)) {
            return Err(Error::Config(
                "--only cannot be used with --last or --only-last.".to_string(),
            ));
        }

        // --- Build content filters vector ---
        let mut content_filters: Vec<Box<dyn ContentFilter>> = Vec::new();
        if self.remove_comments.unwrap_or(false) {
            content_filters.push(Box::new(RemoveCommentsFilter));
        }
        if self.remove_empty_lines.unwrap_or(false) {
            content_filters.push(Box::new(RemoveEmptyLinesFilter));
        }

        let input_path_str = self.input_path.unwrap_or_else(|| ".".to_string());
        let base_path_display: String = input_path_str.clone();

        let (process_last, only_last) = if let Some(only_patterns) = self.only {
            (Some(only_patterns), true)
        } else {
            (self.process_last, self.only_last.unwrap_or(false))
        };

        let git_cache_path =
            Self::determine_cache_dir(self.git_cache_path.as_deref()).map_err(|e| {
                Error::Config(format!("Failed to determine git cache directory: {}", e))
            })?;

        let mut config = Config {
            input_path: PathBuf::new(),
            base_path_display: String::new(),
            input_is_file: false,
            max_size: parse_max_size(self.max_size)?,
            recursive: !self.no_recursive.unwrap_or(false),
            extensions: normalize_extensions(self.extensions),
            exclude_extensions: normalize_extensions(self.exclude_extensions),
            ignore_patterns: self.ignore_patterns,
            exclude_path_regex: compile_regex_vec(self.exclude_path_regex, "exclude path")?,
            path_regex: compile_regex_vec(self.path_regex, "path")?,
            filename_regex: compile_regex_vec(self.filename_regex, "filename")?,
            use_gitignore: !self.no_gitignore.unwrap_or(false),
            include_binary: self.include_binary.unwrap_or(false),
            skip_lockfiles: self.no_lockfiles.unwrap_or(false),
            content_filters,
            filename_only_header: self.filename_only.unwrap_or(false),
            line_numbers: self.line_numbers.unwrap_or(false),
            backticks: self.backticks.unwrap_or(false),
            num_ticks: self.ticks.unwrap_or(3),
            output_destination: if self.paste.unwrap_or(false) {
                OutputDestination::Clipboard
            } else if let Some(file_path_str) = self.output_file {
                OutputDestination::File(PathBuf::from(file_path_str))
            } else {
                OutputDestination::Stdout
            },
            summary: self.summary.unwrap_or(false) || self.counts.unwrap_or(false),
            counts: self.counts.unwrap_or(false),
            process_last,
            only_last,
            dry_run: self.dry_run.unwrap_or(false),
            git_branch: self.git_branch,
            git_depth: self.git_depth,
            git_cache_path,
        };

        let absolute_input_path =
            Self::resolve_and_prepare_input_path(&input_path_str, &config, progress)?;

        config.input_path = absolute_input_path;
        config.base_path_display = base_path_display;
        config.input_is_file = config.input_path.is_file();

        Ok(config)
    }

    /// Determines the absolute path for the git cache directory.
    fn determine_cache_dir(cli_path: Option<&str>) -> AnyhowResult<PathBuf> {
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
        let proj_dirs = ProjectDirs::from("com", "romelium", "dircat")
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

    /// Handles git URLs (cloning or API download) or resolves local paths.
    fn resolve_and_prepare_input_path(
        input_path_str: &str,
        config: &Config,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<PathBuf> {
        if let Some(parsed_url) = git::parse_github_folder_url(input_path_str) {
            log::debug!("Input detected as GitHub folder URL: {:?}", parsed_url);
            Self::handle_github_folder_url(parsed_url, config, progress)
        } else if git::is_git_url(input_path_str) {
            git::get_repo(input_path_str, config, progress).map_err(|e| Error::Git(e.to_string()))
        } else {
            resolve_input_path(input_path_str).map_err(Error::from)
        }
    }

    /// Logic for handling a parsed GitHub folder URL, including API download and fallback to clone.
    fn handle_github_folder_url(
        parsed_url: git::ParsedGitUrl,
        config: &Config,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Result<PathBuf> {
        match git::download_directory_via_api(&parsed_url, config) {
            Ok(temp_dir_root) => {
                log::debug!("Successfully downloaded from GitHub API.");
                let path = temp_dir_root.join(&parsed_url.subdirectory);
                if !path.exists() {
                    return Err(Error::Git(format!(
                        "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                        parsed_url.subdirectory
                    )));
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

                    let cloned_repo_root = git::get_repo(&parsed_url.clone_url, config, progress)
                        .map_err(|e| Error::Git(e.to_string()))?;
                    let path = cloned_repo_root.join(&parsed_url.subdirectory);
                    if !path.exists() {
                        return Err(Error::Git(format!(
                            "Subdirectory '{}' not found in the cloned repository '{}'.",
                            parsed_url.subdirectory, parsed_url.clone_url
                        )));
                    }
                    Ok(path)
                } else {
                    if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                        if reqwest_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                            return Err(Error::Git(format!(
                                "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                                parsed_url.subdirectory
                            )));
                        }
                    }
                    Err(Error::Git(format!(
                        "Failed to download from GitHub API: {}",
                        e
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::errors::Error;
    use clap::Parser;

    #[test]
    fn test_builder_validation_errors() {
        // Conflict: output and paste
        let res1 = ConfigBuilder::new()
            .output_file("f")
            .paste(true)
            .build(None);
        assert!(matches!(res1, Err(Error::Config(_))));
        assert!(res1.unwrap_err().to_string().contains("simultaneously"));

        // Invalid ticks
        let res2 = ConfigBuilder::new().ticks(2).build(None);
        assert!(matches!(res2, Err(Error::Config(_))));
        assert!(res2
            .unwrap_err()
            .to_string()
            .contains("ticks must be 3 or greater"));

        // --only-last without --last
        let res3 = ConfigBuilder::new().only_last(true).build(None);
        assert!(matches!(res3, Err(Error::Config(_))));
        assert!(res3
            .unwrap_err()
            .to_string()
            .contains("--only-last requires --last"));

        // --only with --last
        let res4 = ConfigBuilder::new()
            .only(vec!["*.rs".to_string()])
            .process_last(vec!["*.md".to_string()])
            .build(None);
        assert!(matches!(res4, Err(Error::Config(_))));
        assert!(res4
            .unwrap_err()
            .to_string()
            .contains("--only cannot be used with --last"));
    }

    #[test]
    #[ignore = "requires network access and is slow"]
    fn test_builder_git_clone_error_returns_structured_error() {
        // Use a URL that is syntactically valid but points to a non-existent repo
        let invalid_git_url = "https://github.com/romelium/this-repo-does-not-exist.git";
        let result = ConfigBuilder::new().input_path(invalid_git_url).build(None);

        assert!(matches!(result, Err(Error::Git(_))));
        let err_msg = result.unwrap_err().to_string();
        // Check for a substring that indicates a clone failure
        assert!(err_msg.contains("Failed to clone repository"));
    }

    #[test]
    fn test_builder_basic_config() -> Result<()> {
        let config = ConfigBuilder::new().input_path(".").build(None)?;
        assert!(config.input_path.is_absolute());
        assert_eq!(config.output_destination, OutputDestination::Stdout);
        assert!(config.recursive);
        assert!(config.use_gitignore);
        Ok(())
    }

    #[test]
    fn test_builder_with_flags() -> Result<()> {
        let config = ConfigBuilder::new()
            .input_path(".")
            .no_recursive(true)
            .include_binary(true)
            .build(None)?;
        assert!(!config.recursive);
        assert!(config.include_binary);
        Ok(())
    }

    #[test]
    fn test_builder_only_shorthand_from_cli() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--only", "*.rs", "*.toml"]);
        let config = ConfigBuilder::from_cli(cli).build(None)?;
        assert_eq!(
            config.process_last,
            Some(vec!["*.rs".to_string(), "*.toml".to_string()])
        );
        assert!(config.only_last);
        Ok(())
    }

    #[test]
    fn test_builder_content_filters_from_cli() -> Result<()> {
        // No filters
        let cli_none = Cli::parse_from(["dircat", "."]);
        let config_none = ConfigBuilder::from_cli(cli_none).build(None)?;
        assert!(config_none.content_filters.is_empty());

        // Comments only
        let cli_c = Cli::parse_from(["dircat", ".", "-c"]);
        let config_c = ConfigBuilder::from_cli(cli_c).build(None)?;
        assert_eq!(config_c.content_filters.len(), 1);
        assert_eq!(config_c.content_filters[0].name(), "RemoveCommentsFilter");

        // Empty lines only
        let cli_l = Cli::parse_from(["dircat", ".", "-l"]);
        let config_l = ConfigBuilder::from_cli(cli_l).build(None)?;
        assert_eq!(config_l.content_filters.len(), 1);
        assert_eq!(config_l.content_filters[0].name(), "RemoveEmptyLinesFilter");

        // Both filters
        let cli_cl = Cli::parse_from(["dircat", ".", "-c", "-l"]);
        let config_cl = ConfigBuilder::from_cli(cli_cl).build(None)?;
        assert_eq!(config_cl.content_filters.len(), 2);
        assert_eq!(config_cl.content_filters[0].name(), "RemoveCommentsFilter");
        assert_eq!(
            config_cl.content_filters[1].name(),
            "RemoveEmptyLinesFilter"
        );

        Ok(())
    }
}
