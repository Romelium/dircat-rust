// src/config/builder.rs

//! Builds the `Config` struct from command-line arguments or other sources.
use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    Config, OutputDestination,
};
use crate::cli::Cli;
use crate::errors::{ConfigError, Error, Result};
use crate::processing::filters::{ContentFilter, RemoveCommentsFilter, RemoveEmptyLinesFilter};
use std::path::PathBuf;

/// A builder for creating a `Config` instance from command-line arguments or programmatically.
///
/// This builder handles argument validation and construction of the final `Config` struct
/// after the input path has been resolved.
///
/// # Examples
///
/// ```
/// // Programmatic construction
/// use dircat::config::ConfigBuilder;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///
/// let config = ConfigBuilder::new()
///     .input_path(".")
///     .extensions(vec!["rs".to_string()])
///     .summary(true)
///     .build()
///     .expect("Failed to build config");
///
/// assert!(config.summary);
/// assert_eq!(config.extensions, Some(vec!["rs".to_string()]));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    // --- Input ---
    input_path: Option<String>,
    // --- Git Options ---
    #[cfg(feature = "git")]
    git_branch: Option<String>,
    #[cfg(feature = "git")]
    git_depth: Option<u32>,
    #[cfg(feature = "git")]
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
    content_filters: Vec<Box<dyn ContentFilter>>,
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
            #[cfg(feature = "git")]
            git_branch: cli.git_branch,
            #[cfg(feature = "git")]
            git_depth: cli.git_depth,
            #[cfg(feature = "git")]
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
            content_filters: Vec::new(),
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
    #[cfg(feature = "git")]
    #[must_use]
    pub fn git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }
    #[cfg(feature = "git")]
    #[must_use]
    pub fn git_depth(mut self, depth: u32) -> Self {
        self.git_depth = Some(depth);
        self
    }
    #[cfg(feature = "git")]
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
    pub fn content_filter(mut self, filter: Box<dyn ContentFilter>) -> Self {
        self.content_filters.push(filter);
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
    /// - Parses and compiles patterns (regex, extensions) from the builder's state.
    /// - Uses the pre-resolved path information to construct the final `Config`.
    ///
    /// # Errors
    ///
    /// Returns an error if any validation of option combinations fails.
    pub fn build(self) -> Result<Config> {
        // --- Validation ---
        if self.output_file.is_some() && self.paste.unwrap_or(false) {
            return Err(ConfigError::Conflict {
                option1: "--output".to_string(),
                option2: "--paste".to_string(),
            }
            .into());
        }
        if self.ticks.unwrap_or(3) < 3 {
            return Err(ConfigError::InvalidValue {
                option: "--ticks".to_string(),
                reason: "must be 3 or greater".to_string(),
            }
            .into());
        }
        if self.only_last.unwrap_or(false) && self.process_last.is_none() {
            return Err(ConfigError::MissingDependency {
                option: "--only-last".to_string(),
                required: "--last".to_string(),
            }
            .into());
        }
        if self.only.is_some() && (self.process_last.is_some() || self.only_last.unwrap_or(false)) {
            return Err(ConfigError::Conflict {
                option1: "--only".to_string(),
                option2: "--last or --only-last".to_string(),
            }
            .into());
        }

        // --- Build content filters vector ---
        let mut content_filters = self.content_filters;
        if self.remove_comments.unwrap_or(false) {
            content_filters.push(Box::new(RemoveCommentsFilter));
        }
        if self.remove_empty_lines.unwrap_or(false) {
            content_filters.push(Box::new(RemoveEmptyLinesFilter));
        }

        let (process_last, only_last) = if let Some(only_patterns) = self.only {
            (Some(only_patterns), true)
        } else {
            (self.process_last, self.only_last.unwrap_or(false))
        };

        let config = Config {
            input_path: self.input_path.unwrap_or_else(|| ".".to_string()),
            max_size: parse_max_size(self.max_size).map_err(Error::from)?,
            recursive: !self.no_recursive.unwrap_or(false),
            extensions: normalize_extensions(self.extensions),
            exclude_extensions: normalize_extensions(self.exclude_extensions),
            ignore_patterns: self.ignore_patterns,
            exclude_path_regex: compile_regex_vec(self.exclude_path_regex, "exclude path")
                .map_err(Error::from)?,
            path_regex: compile_regex_vec(self.path_regex, "path").map_err(Error::from)?,
            filename_regex: compile_regex_vec(self.filename_regex, "filename")
                .map_err(Error::from)?,
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
            #[cfg(feature = "git")]
            git_branch: self.git_branch,
            #[cfg(feature = "git")]
            git_depth: self.git_depth,
            #[cfg(feature = "git")]
            git_cache_path: self.git_cache_path,
        };

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::errors::{ConfigError, Error};
    use clap::Parser;

    #[test]
    fn test_builder_validation_errors() {
        // Conflict: output and paste
        let res1 = ConfigBuilder::new().output_file("f").paste(true).build();
        assert!(matches!(
            res1,
            Err(Error::Config(ConfigError::Conflict { .. }))
        ));
        assert!(res1.unwrap_err().to_string().contains("simultaneously"));

        // Invalid ticks
        let res2 = ConfigBuilder::new().ticks(2).build();
        assert!(matches!(
            res2,
            Err(Error::Config(ConfigError::InvalidValue { .. }))
        ));
        assert!(res2
            .unwrap_err()
            .to_string()
            .contains("must be 3 or greater"));

        // --only-last without --last
        let res3 = ConfigBuilder::new().only_last(true).build();
        assert!(matches!(
            res3,
            Err(Error::Config(ConfigError::MissingDependency { .. }))
        ));
        assert!(res3
            .unwrap_err()
            .to_string()
            .contains("requires option '--last'"));

        // --only with --last
        let res4 = ConfigBuilder::new()
            .only(vec!["*.rs".to_string()])
            .process_last(vec!["*.md".to_string()])
            .build();
        assert!(matches!(
            res4,
            Err(Error::Config(ConfigError::Conflict { .. }))
        ));
        assert!(res4
            .unwrap_err()
            .to_string()
            .contains("cannot be used simultaneously"));
    }

    #[test]
    fn test_builder_basic_config() -> Result<()> {
        let config = ConfigBuilder::new().input_path(".").build()?;
        assert_eq!(config.input_path, ".");
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
            .build()?;
        assert!(!config.recursive);
        assert!(config.include_binary);
        Ok(())
    }

    #[test]
    fn test_builder_only_shorthand_from_cli() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--only", "*.rs", "*.toml"]);
        let config = ConfigBuilder::from_cli(cli).build()?;
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
        let config_none = ConfigBuilder::from_cli(cli_none).build()?;
        assert!(config_none.content_filters.is_empty());

        // Comments only
        let cli_c = Cli::parse_from(["dircat", ".", "-c"]);
        let config_c = ConfigBuilder::from_cli(cli_c).build()?;
        assert_eq!(config_c.content_filters.len(), 1);
        assert_eq!(config_c.content_filters[0].name(), "RemoveCommentsFilter");

        // Empty lines only
        let cli_l = Cli::parse_from(["dircat", ".", "-l"]);
        let config_l = ConfigBuilder::from_cli(cli_l).build()?;
        assert_eq!(config_l.content_filters.len(), 1);
        assert_eq!(config_l.content_filters[0].name(), "RemoveEmptyLinesFilter");

        // Both filters
        let cli_cl = Cli::parse_from(["dircat", ".", "-c", "-l"]);
        let config_cl = ConfigBuilder::from_cli(cli_cl).build()?;
        assert_eq!(config_cl.content_filters.len(), 2);
        assert_eq!(config_cl.content_filters[0].name(), "RemoveCommentsFilter");
        assert_eq!(
            config_cl.content_filters[1].name(),
            "RemoveEmptyLinesFilter"
        );

        Ok(())
    }

    // A custom filter for testing purposes
    #[derive(Debug)]
    struct UppercaseFilter;

    impl ContentFilter for UppercaseFilter {
        fn apply(&self, content: &str) -> String {
            content.to_uppercase()
        }
        fn name(&self) -> &'static str {
            "UppercaseFilter"
        }
    }

    #[test]
    fn test_builder_with_custom_content_filter() -> Result<()> {
        let config = ConfigBuilder::new()
            .content_filter(Box::new(UppercaseFilter))
            .build()?;

        assert_eq!(config.content_filters.len(), 1);
        assert_eq!(config.content_filters[0].name(), "UppercaseFilter");
        Ok(())
    }

    #[test]
    fn test_builder_combines_custom_and_cli_filters() -> Result<()> {
        let config = ConfigBuilder::new()
            .remove_comments(true) // Simulates a CLI flag
            .content_filter(Box::new(UppercaseFilter)) // Programmatic custom filter
            .remove_empty_lines(true) // Another CLI flag
            .build()?;

        assert_eq!(config.content_filters.len(), 3);
        // The custom filter is added first from the builder's vec, then the CLI ones are appended.
        assert_eq!(config.content_filters[0].name(), "UppercaseFilter");
        assert_eq!(config.content_filters[1].name(), "RemoveCommentsFilter");
        assert_eq!(config.content_filters[2].name(), "RemoveEmptyLinesFilter");
        Ok(())
    }
}
