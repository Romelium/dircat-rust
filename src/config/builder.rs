// src/config/builder.rs

use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    path_resolve::resolve_input_path,
    validation::validate_cli_options,
    Config, OutputDestination,
};
use crate::cli::Cli;
use crate::git;
use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

/// A builder for creating a `Config` instance from command-line arguments.
///
/// This builder handles argument validation, path resolution, and git repository cloning/downloading.
pub struct ConfigBuilder {
    cli: Cli,
}

impl ConfigBuilder {
    /// Creates a new `ConfigBuilder` from the parsed command-line interface arguments.
    pub fn new(cli: Cli) -> Self {
        Self { cli }
    }

    /// Builds the final `Config` struct.
    ///
    /// This method performs all necessary setup and validation:
    /// - Validates CLI option combinations.
    /// - Parses and compiles patterns (regex, extensions).
    /// - Resolves the git cache path.
    /// - Handles the input path, which can be a local path, a git URL (for cloning),
    ///   or a GitHub folder URL (for API download).
    ///
    /// # Errors
    ///
    /// Returns an error if any validation fails, paths cannot be resolved, or git
    /// operations fail.
    pub fn build(self) -> Result<Config> {
        let cli = self.cli;
        validate_cli_options(&cli)?;

        let base_path_display: String = cli.input_path.clone();

        let (process_last, only_last) = if let Some(only_patterns) = cli.only {
            (Some(only_patterns), true)
        } else {
            (cli.process_last, cli.only_last)
        };

        let git_cache_path = Self::determine_cache_dir(cli.git_cache_path.as_deref())?;

        let mut config = Config {
            input_path: PathBuf::new(),
            base_path_display: String::new(),
            input_is_file: false,
            max_size: parse_max_size(cli.max_size)?,
            recursive: !cli.no_recursive,
            extensions: normalize_extensions(cli.extensions),
            exclude_extensions: normalize_extensions(cli.exclude_extensions),
            ignore_patterns: cli.ignore_patterns,
            exclude_path_regex: compile_regex_vec(cli.exclude_path_regex, "exclude path")?,
            path_regex: compile_regex_vec(cli.path_regex, "path")?,
            filename_regex: compile_regex_vec(cli.filename_regex, "filename")?,
            use_gitignore: !cli.no_gitignore,
            include_binary: cli.include_binary,
            skip_lockfiles: cli.no_lockfiles,
            remove_comments: cli.remove_comments,
            remove_empty_lines: cli.remove_empty_lines,
            filename_only_header: cli.filename_only,
            line_numbers: cli.line_numbers,
            backticks: cli.backticks,
            num_ticks: cli.ticks,
            output_destination: if cli.paste {
                OutputDestination::Clipboard
            } else if let Some(file_path_str) = cli.output_file {
                OutputDestination::File(PathBuf::from(file_path_str))
            } else {
                OutputDestination::Stdout
            },
            summary: cli.summary || cli.counts,
            counts: cli.counts,
            process_last,
            only_last,
            dry_run: cli.dry_run,
            git_branch: cli.git_branch,
            git_depth: cli.git_depth,
            git_cache_path,
        };

        let absolute_input_path = Self::resolve_and_prepare_input_path(&cli.input_path, &config)?;

        config.input_path = absolute_input_path;
        config.base_path_display = base_path_display;
        config.input_is_file = config.input_path.is_file();

        Ok(config)
    }

    /// Determines the absolute path for the git cache directory.
    fn determine_cache_dir(cli_path: Option<&str>) -> Result<PathBuf> {
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
    fn resolve_and_prepare_input_path(input_path_str: &str, config: &Config) -> Result<PathBuf> {
        if let Some(parsed_url) = git::parse_github_folder_url(input_path_str) {
            log::debug!("Input detected as GitHub folder URL: {:?}", parsed_url);
            Self::handle_github_folder_url(parsed_url, config)
        } else if git::is_git_url(input_path_str) {
            git::get_repo(input_path_str, config)
        } else {
            resolve_input_path(input_path_str)
        }
    }

    /// Logic for handling a parsed GitHub folder URL, including API download and fallback to clone.
    fn handle_github_folder_url(parsed_url: git::ParsedGitUrl, config: &Config) -> Result<PathBuf> {
        match git::download_directory_via_api(&parsed_url, config) {
            Ok(temp_dir_root) => {
                log::debug!("Successfully downloaded from GitHub API.");
                let path = temp_dir_root.join(&parsed_url.subdirectory);
                if !path.exists() {
                    return Err(anyhow!(
                        "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                        parsed_url.subdirectory
                    ));
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

                    let cloned_repo_root = git::get_repo(&parsed_url.clone_url, config)?;
                    let path = cloned_repo_root.join(&parsed_url.subdirectory);
                    if !path.exists() {
                        return Err(anyhow!(
                            "Subdirectory '{}' not found in the cloned repository '{}'.",
                            parsed_url.subdirectory,
                            parsed_url.clone_url
                        ));
                    }
                    Ok(path)
                } else {
                    if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                        if reqwest_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                            return Err(anyhow!(
                                "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                                parsed_url.subdirectory
                            ));
                        }
                    }
                    Err(anyhow!("Failed to download from GitHub API: {}", e))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn test_builder_basic_config() -> Result<()> {
        let cli = Cli::parse_from(["dircat", "."]);
        let config = ConfigBuilder::new(cli).build()?;
        assert!(config.input_path.is_absolute());
        assert_eq!(config.output_destination, OutputDestination::Stdout);
        assert!(config.recursive);
        assert!(config.use_gitignore);
        Ok(())
    }

    #[test]
    fn test_builder_with_flags() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--no-recursive", "--include-binary"]);
        let config = ConfigBuilder::new(cli).build()?;
        assert!(!config.recursive);
        assert!(config.include_binary);
        Ok(())
    }

    #[test]
    fn test_builder_only_shorthand() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--only", "*.rs", "*.toml"]);
        let config = ConfigBuilder::new(cli).build()?;
        assert_eq!(
            config.process_last,
            Some(vec!["*.rs".to_string(), "*.toml".to_string()])
        );
        assert!(config.only_last);
        Ok(())
    }
}
