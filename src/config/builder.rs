// src/config/builder.rs

//! Builds the `Config` struct from command-line arguments or other sources.
use super::{
    parsing::{compile_regex_vec, normalize_extensions, parse_max_size},
    Config, DiscoveryConfig, OutputConfig, ProcessingConfig,
};
use crate::cli::Cli;
use crate::errors::{Error, Result};
use crate::processing::filters::ContentFilter;

use super::builder_logic;

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
/// assert!(config.output.summary);
/// assert_eq!(config.discovery.extensions, Some(vec!["rs".to_string()]));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone)]
pub struct ConfigBuilder {
    // --- Input ---
    pub(crate) input_path: Option<String>,
    // --- Git Options ---
    #[cfg(feature = "git")]
    pub(crate) git_branch: Option<String>,
    #[cfg(feature = "git")]
    pub(crate) git_depth: Option<u32>,
    #[cfg(feature = "git")]
    pub(crate) git_cache_path: Option<String>,
    // --- Filtering Options ---
    pub(crate) max_size: Option<String>,
    pub(crate) no_recursive: Option<bool>,
    pub(crate) extensions: Option<Vec<String>>,
    pub(crate) exclude_extensions: Option<Vec<String>>,
    pub(crate) exclude_path_regex: Option<Vec<String>>,
    pub(crate) ignore_patterns: Option<Vec<String>>,
    pub(crate) path_regex: Option<Vec<String>>,
    pub(crate) filename_regex: Option<Vec<String>>,
    pub(crate) no_gitignore: Option<bool>,
    pub(crate) include_binary: Option<bool>,
    pub(crate) no_lockfiles: Option<bool>,
    // --- Content Processing Options ---
    pub(crate) remove_comments: Option<bool>,
    pub(crate) remove_empty_lines: Option<bool>,
    pub(crate) content_filters: Vec<Box<dyn ContentFilter>>,
    // --- Output Formatting Options ---
    pub(crate) filename_only: Option<bool>,
    pub(crate) line_numbers: Option<bool>,
    pub(crate) backticks: Option<bool>,
    pub(crate) ticks: Option<u8>,
    // --- Output Destination & Summary ---
    pub(crate) output_file: Option<String>,
    #[cfg(feature = "clipboard")]
    pub(crate) paste: Option<bool>,
    pub(crate) summary: Option<bool>,
    pub(crate) counts: Option<bool>,
    // --- Processing Order ---
    pub(crate) process_last: Option<Vec<String>>,
    pub(crate) only_last: Option<bool>,
    pub(crate) only: Option<Vec<String>>,
    // --- Execution Control ---
    pub(crate) dry_run: Option<bool>,
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
            #[cfg(feature = "clipboard")]
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
    /// Sets the input path (directory, file, or git URL).
    ///
    /// # Examples
    ///
    /// ```
    /// # use dircat::config::ConfigBuilder;
    /// let config = ConfigBuilder::new().input_path("/path/to/dir").build().unwrap();
    /// assert_eq!(config.input_path, "/path/to/dir");
    /// ```
    #[must_use]
    pub fn input_path(mut self, path: impl Into<String>) -> Self {
        self.input_path = Some(path.into());
        self
    }

    /// Sets the git branch, tag, or commit to check out.
    #[cfg(feature = "git")]
    #[must_use]
    pub fn git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }

    /// Sets the depth for a shallow git clone.
    #[cfg(feature = "git")]
    #[must_use]
    pub fn git_depth(mut self, depth: u32) -> Self {
        self.git_depth = Some(depth);
        self
    }

    /// Sets the path for caching cloned git repositories.
    #[cfg(feature = "git")]
    #[must_use]
    pub fn git_cache_path(mut self, path: impl Into<String>) -> Self {
        self.git_cache_path = Some(path.into());
        self
    }

    /// Sets the maximum file size to include (e.g., "1M", "512k").
    #[must_use]
    pub fn max_size(mut self, size: impl Into<String>) -> Self {
        self.max_size = Some(size.into());
        self
    }

    /// Disables recursive directory traversal if `true`.
    #[must_use]
    pub fn no_recursive(mut self, no_recurse: bool) -> Self {
        self.no_recursive = Some(no_recurse);
        self
    }

    /// Sets the list of file extensions to include.
    #[must_use]
    pub fn extensions(mut self, exts: Vec<String>) -> Self {
        self.extensions = Some(exts);
        self
    }

    /// Sets the list of file extensions to exclude.
    #[must_use]
    pub fn exclude_extensions(mut self, exts: Vec<String>) -> Self {
        self.exclude_extensions = Some(exts);
        self
    }

    /// Sets the list of regular expressions for excluding file paths.
    #[must_use]
    pub fn exclude_path_regex(mut self, regexes: Vec<String>) -> Self {
        self.exclude_path_regex = Some(regexes);
        self
    }

    /// Sets the list of custom glob patterns to ignore.
    #[must_use]
    pub fn ignore_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = Some(patterns);
        self
    }

    /// Sets the list of regular expressions for including file paths.
    #[must_use]
    pub fn path_regex(mut self, regexes: Vec<String>) -> Self {
        self.path_regex = Some(regexes);
        self
    }

    /// Sets the list of regular expressions for including filenames.
    #[must_use]
    pub fn filename_regex(mut self, regexes: Vec<String>) -> Self {
        self.filename_regex = Some(regexes);
        self
    }

    /// Disables `.gitignore` processing if `true`.
    #[must_use]
    pub fn no_gitignore(mut self, no_gitignore: bool) -> Self {
        self.no_gitignore = Some(no_gitignore);
        self
    }

    /// Includes binary files in the output if `true`.
    #[must_use]
    pub fn include_binary(mut self, include: bool) -> Self {
        self.include_binary = Some(include);
        self
    }

    /// Skips common lockfiles if `true`.
    #[must_use]
    pub fn no_lockfiles(mut self, no_lockfiles: bool) -> Self {
        self.no_lockfiles = Some(no_lockfiles);
        self
    }

    /// Enables removal of C-style comments if `true`.
    #[must_use]
    pub fn remove_comments(mut self, remove: bool) -> Self {
        self.remove_comments = Some(remove);
        self
    }

    /// Enables removal of empty lines if `true`.
    #[must_use]
    pub fn remove_empty_lines(mut self, remove: bool) -> Self {
        self.remove_empty_lines = Some(remove);
        self
    }

    /// Adds a custom content filter to the processing pipeline.
    #[must_use]
    pub fn content_filter(mut self, filter: Box<dyn ContentFilter>) -> Self {
        self.content_filters.push(filter);
        self
    }

    /// Displays only the filename in headers if `true`.
    #[must_use]
    pub fn filename_only(mut self, filename_only: bool) -> Self {
        self.filename_only = Some(filename_only);
        self
    }

    /// Adds line numbers to the output if `true`.
    #[must_use]
    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = Some(line_numbers);
        self
    }

    /// Wraps filenames in backticks if `true`.
    #[must_use]
    pub fn backticks(mut self, backticks: bool) -> Self {
        self.backticks = Some(backticks);
        self
    }

    /// Sets the number of backticks for Markdown code fences.
    #[must_use]
    pub fn ticks(mut self, count: u8) -> Self {
        self.ticks = Some(count);
        self
    }

    /// Sets the output file path.
    #[must_use]
    pub fn output_file(mut self, path: impl Into<String>) -> Self {
        self.output_file = Some(path.into());
        self
    }

    /// Copies the output to the clipboard if `true`.
    #[cfg(feature = "clipboard")]
    #[must_use]
    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = Some(paste);
        self
    }

    /// Appends a summary of processed files if `true`.
    #[must_use]
    pub fn summary(mut self, summary: bool) -> Self {
        self.summary = Some(summary);
        self
    }

    /// Includes file counts in the summary if `true`.
    #[must_use]
    pub fn counts(mut self, counts: bool) -> Self {
        self.counts = Some(counts);
        self
    }

    /// Sets the list of glob patterns for files to be processed last.
    #[must_use]
    pub fn process_last(mut self, patterns: Vec<String>) -> Self {
        self.process_last = Some(patterns);
        self
    }

    /// Processes only the files matching `process_last` patterns if `true`.
    #[must_use]
    pub fn only_last(mut self, only_last: bool) -> Self {
        self.only_last = Some(only_last);
        self
    }

    /// Sets the list of glob patterns for files to be processed, skipping all others.
    #[must_use]
    pub fn only(mut self, patterns: Vec<String>) -> Self {
        self.only = Some(patterns);
        self
    }

    /// Performs a dry run if `true`, listing files without their content.
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
    /// Returns an error if any validation of option combinations fails, or if
    /// parsing of values like max size or regex patterns fails.
    pub fn build(self) -> Result<Config> {
        builder_logic::validate_builder_options(&self)?;

        let content_filters = builder_logic::build_content_filters(
            self.content_filters,
            self.remove_comments,
            self.remove_empty_lines,
        );

        let (process_last, only_last) =
            builder_logic::determine_process_order(self.only, self.process_last, self.only_last);

        let output_destination = builder_logic::determine_output_destination(
            self.output_file,
            #[cfg(feature = "clipboard")]
            self.paste,
        );

        let discovery_config = DiscoveryConfig {
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
            skip_lockfiles: self.no_lockfiles.unwrap_or(false),
            process_last,
            only_last,
        };

        let processing_config = ProcessingConfig {
            include_binary: self.include_binary.unwrap_or(false),
            counts: self.counts.unwrap_or(false),
            content_filters,
        };

        let output_config = OutputConfig {
            filename_only_header: self.filename_only.unwrap_or(false),
            line_numbers: self.line_numbers.unwrap_or(false),
            backticks: self.backticks.unwrap_or(false),
            num_ticks: self.ticks.unwrap_or(3),
            summary: self.summary.unwrap_or(false) || self.counts.unwrap_or(false),
            counts: self.counts.unwrap_or(false),
        };

        let config = Config {
            input_path: self.input_path.unwrap_or_else(|| ".".to_string()),
            discovery: discovery_config,
            processing: processing_config,
            output: output_config,
            output_destination,
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
    use crate::config::OutputDestination;
    use crate::errors::{ConfigError, Error};
    use clap::Parser;

    #[test]
    fn test_builder_validation_errors() {
        // Conflict: output and paste
        #[cfg(feature = "clipboard")]
        {
            let res1 = ConfigBuilder::new().output_file("f").paste(true).build();
            assert!(matches!(
                res1,
                Err(Error::Config(ConfigError::Conflict { .. }))
            ));
            assert!(res1.unwrap_err().to_string().contains("simultaneously"));
        }
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
        assert!(config.discovery.recursive);
        assert!(config.discovery.use_gitignore);
        Ok(())
    }
    #[test]
    fn test_builder_with_flags() -> Result<()> {
        let config = ConfigBuilder::new()
            .input_path(".")
            .no_recursive(true)
            .include_binary(true)
            .build()?;
        assert!(!config.discovery.recursive);
        assert!(config.processing.include_binary);
        Ok(())
    }

    #[test]
    fn test_builder_only_shorthand_from_cli() -> Result<()> {
        let cli = Cli::parse_from(["dircat", ".", "--only", "*.rs", "*.toml"]);
        let config = ConfigBuilder::from_cli(cli).build()?;
        assert_eq!(
            config.discovery.process_last,
            Some(vec!["*.rs".to_string(), "*.toml".to_string()])
        );
        assert!(config.discovery.only_last);
        Ok(())
    }

    #[test]
    fn test_builder_content_filters_from_cli() -> Result<()> {
        // No filters
        let cli_none = Cli::parse_from(["dircat", "."]);
        let config_none = ConfigBuilder::from_cli(cli_none).build()?;
        assert!(config_none.processing.content_filters.is_empty());

        // Comments only
        let cli_c = Cli::parse_from(["dircat", ".", "-c"]);
        let config_c = ConfigBuilder::from_cli(cli_c).build()?;
        assert_eq!(config_c.processing.content_filters.len(), 1);
        assert_eq!(
            config_c.processing.content_filters[0].name(),
            "RemoveCommentsFilter"
        );

        // Empty lines only
        let cli_l = Cli::parse_from(["dircat", ".", "-l"]);
        let config_l = ConfigBuilder::from_cli(cli_l).build()?;
        assert_eq!(config_l.processing.content_filters.len(), 1);
        assert_eq!(
            config_l.processing.content_filters[0].name(),
            "RemoveEmptyLinesFilter"
        );

        // Both filters
        let cli_cl = Cli::parse_from(["dircat", ".", "-c", "-l"]);
        let config_cl = ConfigBuilder::from_cli(cli_cl).build()?;
        assert_eq!(config_cl.processing.content_filters.len(), 2);
        assert_eq!(
            config_cl.processing.content_filters[0].name(),
            "RemoveCommentsFilter"
        );
        assert_eq!(
            config_cl.processing.content_filters[1].name(),
            "RemoveEmptyLinesFilter"
        );

        Ok(())
    }

    // A custom filter for testing purposes
    #[derive(Debug, Clone)]
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

        assert_eq!(config.processing.content_filters.len(), 1);
        assert_eq!(
            config.processing.content_filters[0].name(),
            "UppercaseFilter"
        );
        Ok(())
    }

    #[test]
    fn test_builder_combines_custom_and_cli_filters() -> Result<()> {
        let config = ConfigBuilder::new()
            .remove_comments(true) // Simulates a CLI flag
            .content_filter(Box::new(UppercaseFilter)) // Programmatic custom filter
            .remove_empty_lines(true) // Another CLI flag
            .build()?;

        assert_eq!(config.processing.content_filters.len(), 3);
        // The custom filter is added first from the builder's vec, then the CLI ones are appended.
        assert_eq!(
            config.processing.content_filters[0].name(),
            "UppercaseFilter"
        );
        assert_eq!(
            config.processing.content_filters[1].name(),
            "RemoveCommentsFilter"
        );
        assert_eq!(
            config.processing.content_filters[2].name(),
            "RemoveEmptyLinesFilter"
        );
        Ok(())
    }
}
