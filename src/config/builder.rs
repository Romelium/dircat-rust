use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    path_resolve::resolve_input_path,
    validation::validate_cli_options,
    Config, OutputDestination,
};
use crate::cli::Cli;
use crate::git;
use anyhow::{anyhow, Result};
use std::path::PathBuf; // Keep PathBuf

impl TryFrom<Cli> for Config {
    type Error = anyhow::Error;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        validate_cli_options(&cli)?;
        let absolute_input_path: PathBuf;
        let base_path_display: String = cli.input_path.clone();

        let mut config = Config {
            // Initialize with defaults that will be overwritten
            input_path: PathBuf::new(),
            base_path_display: String::new(), // Will be overwritten later
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
            output_destination: if cli.paste {
                OutputDestination::Clipboard
            } else if let Some(file_path_str) = cli.output_file {
                OutputDestination::File(PathBuf::from(file_path_str))
            } else {
                OutputDestination::Stdout
            },
            summary: cli.summary || cli.counts,
            counts: cli.counts,
            process_last: cli.process_last,
            only_last: cli.only_last,
            dry_run: cli.dry_run,
            git_branch: cli.git_branch,
            git_depth: cli.git_depth,
        };

        // Check if the input is a GitHub folder URL
        if let Some(parsed_url) = git::parse_github_folder_url(&cli.input_path) {
            log::debug!("Input detected as GitHub folder URL: {:?}", parsed_url);

            match git::download_directory_via_api(&parsed_url, &config) {
                Ok(temp_dir_root) => {
                    // API download successful, proceed as before.
                    log::debug!("Successfully downloaded from GitHub API.");
                    // The actual path to process is the subdirectory *within* the temporary directory.
                    absolute_input_path = temp_dir_root.join(&parsed_url.subdirectory);
                    if !absolute_input_path.exists() {
                        return Err(anyhow!(
                            "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                            parsed_url.subdirectory
                        ));
                    }
                }
                Err(e) => {
                    // API download failed, check for rate limit error.
                    let is_rate_limit_error =
                        e.downcast_ref::<reqwest::Error>().map_or(false, |re| {
                            re.status() == Some(reqwest::StatusCode::FORBIDDEN)
                        });

                    if is_rate_limit_error {
                        // It's a 403 FORBIDDEN error, so we fall back to a full git clone.
                        log::warn!(
                            "GitHub API request failed (likely rate-limited). Falling back to a full git clone of '{}'.",
                            parsed_url.clone_url
                        );
                        log::warn!("To avoid this, set a GITHUB_TOKEN environment variable with 'repo' scope.");

                        // Use the clone_url from the parsed URL to clone the whole repo.
                        let cloned_repo_root = git::get_repo(&parsed_url.clone_url, &config)?;

                        // The path to process is the subdirectory *within* the cloned repo.
                        absolute_input_path = cloned_repo_root.join(&parsed_url.subdirectory);

                        // Check if the subdirectory actually exists in the cloned repo.
                        if !absolute_input_path.exists() {
                            return Err(anyhow!(
                                "Subdirectory '{}' not found in the cloned repository '{}'.",
                                parsed_url.subdirectory,
                                parsed_url.clone_url
                            ));
                        }
                    } else {
                        // It's some other API error (like 404 NOT_FOUND), handle as before.
                        if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                            if reqwest_err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                                return Err(anyhow!(
                                    "Subdirectory '{}' not found in repository. It might be empty or invalid.",
                                    parsed_url.subdirectory
                                ));
                            }
                        }
                        return Err(anyhow!("Failed to download from GitHub API: {}", e));
                    }
                }
            }
        } else if git::is_git_url(&cli.input_path) {
            // Standard git URL (non-github, e.g., gitlab) uses the clone method
            absolute_input_path = git::get_repo(&cli.input_path, &config)?;
        } else {
            // Local path
            absolute_input_path = resolve_input_path(&cli.input_path)?;
        }

        config.input_path = absolute_input_path;
        config.base_path_display = base_path_display;
        config.input_is_file = config.input_path.is_file();

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn test_basic_config_creation() -> Result<()> {
        let cli = Cli::parse_from(["dircat", "."]);
        let config = Config::try_from(cli)?;
        assert!(config.input_path.is_absolute());
        assert_eq!(config.output_destination, OutputDestination::Stdout);
        assert!(config.recursive);
        assert!(config.use_gitignore);
        assert!(!config.input_is_file); // Current directory is not a file
        assert!(!config.include_binary); // Default is false
        assert!(!config.skip_lockfiles); // Default is false
        Ok(())
    }

    #[test]
    fn test_include_binary_flag() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--include-binary"]);
        let config = Config::try_from(cli)?;
        assert!(config.include_binary);
        Ok(())
    }

    #[test]
    fn test_no_lockfiles_flag() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--no-lockfiles"]);
        let config = Config::try_from(cli)?;
        assert!(config.skip_lockfiles);
        Ok(())
    }

    #[test]
    fn test_counts_implies_summary() -> Result<()> {
        // Test that --counts alone implies --summary.
        let cli = Cli::parse_from(["dircat", ".", "--counts"]);
        let config = Config::try_from(cli)?;
        assert!(config.summary);
        assert!(config.counts);

        // Test that --summary alone does not imply --counts.
        let cli_summary_only = Cli::parse_from(["dircat", ".", "--summary"]);
        let config_summary_only = Config::try_from(cli_summary_only)?;
        assert!(config_summary_only.summary);
        assert!(!config_summary_only.counts);

        Ok(())
    }

    #[test]
    fn test_only_last_requires_last_clap() {
        // This should be caught by clap's `requires` attribute
        let result = Cli::try_parse_from(["dircat", ".", "-Z"]);
        assert!(result.is_err());
        // Check for key parts of the actual error message
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("required arguments were not provided") && error_message.contains("--last"),
            "Expected error message to indicate '--last' was required but not provided, but got: {}", error_message
        );
    }
}
