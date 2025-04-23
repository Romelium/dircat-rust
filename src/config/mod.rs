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
    pub input_is_file: bool, // <-- ADDED FLAG
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
    /// List of compiled regular expressions to match against the full file path. If `Some`, the path must match at least one.
    pub path_regex: Option<Vec<Regex>>,
    /// List of compiled regular expressions to match against the filename (basename). If `Some`, the filename must match at least one.
    pub filename_regex: Option<Vec<Regex>>,
    /// Whether to respect `.gitignore`, `.ignore`, and other VCS ignore files.
    pub use_gitignore: bool,
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
