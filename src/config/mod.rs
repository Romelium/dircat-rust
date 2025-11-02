use regex::Regex;
use std::path::PathBuf;

pub mod builder;
mod parsing;
mod path_resolve;
mod validation;

/// Internal configuration derived from command-line arguments.
///
/// This struct holds all the settings parsed and validated from the CLI,
/// ready to be used by the core logic (discovery, processing, output).
#[derive(Debug)]
pub struct Config {
    /// The resolved, absolute path to the input directory or file.
    pub input_path: PathBuf,
    /// The original input path string provided by the user, used for display purposes
    /// and potentially for calculating relative paths if `input_path` stripping fails.
    pub base_path_display: String,
    /// Flag indicating if the resolved input_path points to a file rather than a directory.
    pub input_is_file: bool,
    /// Maximum file size in bytes. Files larger than this will be skipped.
    pub max_size: Option<u128>, // Changed from u64 to u128
    /// Whether to recurse into subdirectories.
    pub recursive: bool,
    /// List of file extensions (lowercase) to include. If `Some`, only files with these extensions are processed.
    pub extensions: Option<Vec<String>>,
    /// List of file extensions (lowercase) to exclude. Takes precedence over `extensions`.
    pub exclude_extensions: Option<Vec<String>>,
    /// List of custom ignore patterns (gitignore syntax) provided via `-i`.
    pub ignore_patterns: Option<Vec<String>>,
    /// List of compiled regular expressions to exclude files by full path. If `Some`, any path matching one of these is skipped.
    pub exclude_path_regex: Option<Vec<Regex>>,
    /// List of compiled regular expressions to match against the full file path. If `Some`, the path must match at least one.
    pub path_regex: Option<Vec<Regex>>,
    /// List of compiled regular expressions to match against the filename (basename). If `Some`, the filename must match at least one.
    pub filename_regex: Option<Vec<Regex>>,
    /// Whether to respect `.gitignore`, `.ignore`, and other VCS ignore files.
    pub use_gitignore: bool,
    /// Whether to include files detected as binary/non-text.
    pub include_binary: bool,
    /// Whether to skip common lockfiles.
    pub skip_lockfiles: bool,
    /// Whether to remove C/C++ style comments (`//`, `/* ... */`) from file content.
    pub remove_comments: bool,
    /// Whether to remove lines containing only whitespace from file content.
    pub remove_empty_lines: bool,
    /// Whether to display only the filename (basename) in the `## File:` header instead of the relative path.
    pub filename_only_header: bool,
    /// Whether to add line numbers (`N | `) to the output.
    pub line_numbers: bool,
    /// Whether to wrap filenames in backticks (`) in headers and the summary.
    pub backticks: bool,
    /// Number of backticks for Markdown code fences.
    pub num_ticks: u8,
    /// Specifies where the final output should be written.
    pub output_destination: OutputDestination,
    /// Whether to print a summary list of processed files at the end. Implied by `counts`.
    pub summary: bool,
    /// Whether to calculate and display line, character, and word counts in the summary.
    pub counts: bool,
    /// List of patterns (relative paths or filenames) for files to be processed last, in the specified order.
    pub process_last: Option<Vec<String>>,
    /// If `true`, only process files matching the `process_last` patterns.
    pub only_last: bool,
    /// If `true`, perform a dry run: print the list of files that would be processed, but not their content.
    pub dry_run: bool,
    /// The specific git branch to clone.
    pub git_branch: Option<String>,
    /// The depth for a shallow git clone.
    pub git_depth: Option<u32>,
}

#[cfg(test)]
impl Config {
    /// Creates a default `Config` for testing purposes.
    pub fn new_for_test() -> Self {
        Self {
            input_path: PathBuf::from("."),
            base_path_display: ".".to_string(),
            input_is_file: false,
            max_size: None,
            recursive: true,
            extensions: None,
            exclude_extensions: None,
            ignore_patterns: None,
            path_regex: None,
            exclude_path_regex: None,
            filename_regex: None,
            use_gitignore: true,
            include_binary: false,
            skip_lockfiles: false,
            remove_comments: false,
            remove_empty_lines: false,
            filename_only_header: false,
            line_numbers: false,
            backticks: false,
            num_ticks: 3,
            output_destination: OutputDestination::Stdout,
            summary: false,
            counts: false,
            process_last: None,
            only_last: false,
            dry_run: false,
            git_branch: None,
            git_depth: None,
        }
    }
}

/// Represents the destination for the generated output.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OutputDestination {
    /// Write to standard output.
    Stdout,
    /// Write to the specified file path.
    File(PathBuf),
    /// Copy the output to the system clipboard (requires the `clipboard` feature).
    Clipboard,
}
