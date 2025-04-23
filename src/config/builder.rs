// src/config/builder.rs

use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    path_resolve::resolve_input_path,
    validation::validate_cli_options,
    Config, OutputDestination,
};
use crate::cli::Cli;
use anyhow::Result;
use std::path::PathBuf;

impl TryFrom<Cli> for Config {
    type Error = anyhow::Error;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        validate_cli_options(&cli)?;

        let absolute_input_path = resolve_input_path(&cli.input_path)?;
        // Determine if the resolved path is a file
        let input_is_file = absolute_input_path.is_file();

        let max_size = parse_max_size(cli.max_size)?;
        let path_regex = compile_regex_vec(cli.path_regex, "path")?;
        let filename_regex = compile_regex_vec(cli.filename_regex, "filename")?;
        let extensions = normalize_extensions(cli.extensions);
        let exclude_extensions = normalize_extensions(cli.exclude_extensions);

        let output_destination = if cli.paste {
            OutputDestination::Clipboard
        } else if let Some(file_path_str) = cli.output_file {
            OutputDestination::File(PathBuf::from(file_path_str))
        } else {
            OutputDestination::Stdout
        };

        Ok(Config {
            input_path: absolute_input_path,
            base_path_display: cli.input_path, // Store original for display
            input_is_file,                     // <-- ADDED: Set the flag based on the check
            max_size,
            recursive: !cli.no_recursive,
            extensions,
            exclude_extensions,
            ignore_patterns: cli.ignore_patterns,
            path_regex,
            filename_regex,
            use_gitignore: !cli.no_gitignore,
            remove_comments: cli.remove_comments,
            remove_empty_lines: cli.remove_empty_lines,
            filename_only_header: cli.filename_only,
            line_numbers: cli.line_numbers,
            backticks: cli.backticks,
            output_destination,
            summary: cli.summary || cli.counts, // --counts implies -s
            counts: cli.counts,
            process_last: cli.process_last,
            only_last: cli.only_last,
            dry_run: cli.dry_run,
        })
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
