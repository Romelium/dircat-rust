// src/cli.rs

use clap::Parser;
// use std::path::PathBuf; // Not used directly here

/// High-performance Rust utility that concatenates and displays directory contents,
/// respecting .gitignore rules and offering various filtering/formatting options.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the directory or file to process.
    #[arg(default_value = ".")]
    pub input_path: String,

    // --- Filtering Options ---
    /// Maximum file size to include (e.g., "1M", "512k", "1024"). Files larger than this are skipped.
    #[arg(short = 'm', long, value_name = "BYTES")]
    pub max_size: Option<String>, // Will be parsed into u64 later

    /// Do not recurse into subdirectories. Process only the top-level directory or the specified file.
    #[arg(short = 'n', long, action = clap::ArgAction::SetTrue)]
    pub no_recursive: bool,

    /// Include only files with these extensions (case-insensitive). Can be specified multiple times.
    #[arg(short = 'e', long = "ext", value_name = "EXT", num_args = 1..)]
    pub extensions: Option<Vec<String>>,

    /// Exclude files with these extensions (case-insensitive). Can be specified multiple times.
    #[arg(short = 'x', long = "exclude-ext", value_name = "EXT", num_args = 1..)]
    pub exclude_extensions: Option<Vec<String>>,

    /// Ignore files or directories matching these glob patterns (relative to input path). Can be specified multiple times.
    #[arg(short = 'i', long = "ignore", value_name = "GLOB", num_args = 1..)]
    pub ignore_patterns: Option<Vec<String>>,

    /// Include only files whose full path matches any of these regular expressions (case-insensitive).
    #[arg(short = 'r', long = "regex", value_name = "REGEX", num_args = 1..)]
    pub path_regex: Option<Vec<String>>,

    /// Include only files whose filename (basename) matches any of these regular expressions (case-insensitive).
    #[arg(short = 'd', long = "filename-regex", value_name = "REGEX", num_args = 1..)]
    pub filename_regex: Option<Vec<String>>,

    /// Do not respect .gitignore, .ignore, or other VCS ignore files. Process all files found.
    #[arg(short = 't', long, action = clap::ArgAction::SetTrue)]
    pub no_gitignore: bool,

    /// Include files detected as binary/non-text (default is to skip them).
    #[arg(short = 'B', long, action = clap::ArgAction::SetTrue)] // <-- ADDED short = 'B'
    pub include_binary: bool,

    /// Skip common lockfiles (e.g., Cargo.lock, package-lock.json).
    #[arg(short = 'K', long, action = clap::ArgAction::SetTrue)] // <-- ADDED short = 'K'
    pub no_lockfiles: bool,

    // --- Content Processing Options ---
    /// Remove C/C++ style comments (// and /* ... */) from the output.
    #[arg(short = 'c', long, action = clap::ArgAction::SetTrue)]
    pub remove_comments: bool,

    /// Remove empty lines (containing only whitespace) from the output.
    #[arg(short = 'l', long, action = clap::ArgAction::SetTrue)]
    pub remove_empty_lines: bool,

    // --- Output Formatting Options ---
    /// Only include the filename (basename) in the '## File:' header, not the full relative path.
    #[arg(short = 'f', long, action = clap::ArgAction::SetTrue)]
    pub filename_only: bool,

    /// Add line numbers (N | ) to the beginning of each line in the code blocks.
    #[arg(short = 'L', long, action = clap::ArgAction::SetTrue)]
    pub line_numbers: bool,

    /// Wrap filenames in the '## File:' header and summary list with backticks (`).
    #[arg(short = 'b', long, action = clap::ArgAction::SetTrue)]
    pub backticks: bool,

    // --- Output Destination & Summary ---
    /// Write the output to the specified file instead of stdout.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output_file: Option<String>, // Using String, convert to PathBuf later

    /// Copy the output to the system clipboard instead of printing to stdout or a file. Requires 'clipboard' feature.
    #[arg(short = 'p', long, action = clap::ArgAction::SetTrue)]
    #[cfg_attr(feature = "clipboard", arg(requires = "clipboard_feature"))]
    // Conditionally add requires
    pub paste: bool,

    /// Print a summary list of processed files at the end of the output.
    #[arg(short = 's', long, action = clap::ArgAction::SetTrue)]
    pub summary: bool,

    /// Include line, character (byte), and word counts in the summary (implies -s).
    #[arg(short = 'C', long, action = clap::ArgAction::SetTrue, requires = "summary")]
    // <-- ADDED short = 'C'
    pub counts: bool,

    // --- Processing Order ---
    /// Process files matching these glob patterns (relative path/filename) last, in the order specified.
    #[arg(short = 'z', long = "last", value_name = "GLOB", num_args = 1..)]
    pub process_last: Option<Vec<String>>,

    /// Only process the files specified with -z/--last. Skip all other files.
    #[arg(short = 'Z', long, action = clap::ArgAction::SetTrue, requires = "process_last")]
    pub only_last: bool,

    // --- Execution Control ---
    /// Perform a dry run. Print the files that would be processed (respecting filters and order) but not their content.
    #[arg(short = 'D', long, action = clap::ArgAction::SetTrue)]
    pub dry_run: bool,

    // --- Feature Flags (for conditional compilation/requires) ---
    /// Dummy argument to enforce clipboard feature requirement for -p. Not intended for user interaction.
    #[arg(long, hide = true, action = clap::ArgAction::SetTrue)]
    #[cfg(feature = "clipboard")]
    clipboard_feature: bool, // This argument only exists if the feature is enabled
}
