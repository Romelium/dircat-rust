// src/config/builder.rs

use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    path_resolve::resolve_input_path,
    validation::validate_cli_options,
    Config, OutputDestination,
};
use crate::cli::Cli;
use crate::git;
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

impl TryFrom<Cli> for Config {
    type Error = anyhow::Error;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        validate_cli_options(&cli)?;

        let mut _temp_dir: Option<TempDir> = None;
        let absolute_input_path: PathBuf;
        let base_path_display: String;

        let mut config = Config {
            // Initialize with defaults that will be overwritten
            input_path: PathBuf::new(),
            base_path_display: String::new(),
            input_is_file: false,
            max_size: parse_max_size(cli.max_size)?,
            recursive: !cli.no_recursive,
            extensions: normalize_extensions(cli.extensions),
            exclude_extensions: normalize_extensions(cli.exclude_extensions),
            ignore_patterns: cli.ignore_patterns,
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
            _temp_dir: None,
        };

        // Check if the input is a git URL
        if git::is_git_url(&cli.input_path) {
            let (temp_dir_holder, cloned_path) = git::clone_repo(&cli.input_path, &config)?;
            _temp_dir = Some(temp_dir_holder);
            absolute_input_path = cloned_path;
            base_path_display = cli.input_path;
        } else {
            // Otherwise, treat it as a local path
            absolute_input_path = resolve_input_path(&cli.input_path)?;
            base_path_display = cli.input_path;
        }

        config.input_path = absolute_input_path;
        config.base_path_display = base_path_display;
        config.input_is_file = config.input_path.is_file();
        config._temp_dir = _temp_dir;

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
        assert!(config._temp_dir.is_none()); // Should be none for local path
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
        // Parse with both flags to satisfy clap's `requires` rule
        let cli = Cli::parse_from(["dircat", ".", "--counts", "--summary"]);
        let config = Config::try_from(cli)?;
        // Check that the resulting Config has both flags set correctly
        assert!(config.summary); // Should be true because --summary was passed AND because --counts implies it
        assert!(config.counts); // Should be true because --counts was passed
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
