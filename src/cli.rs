// src/cli.rs

use clap::Parser;

/// A high-performance Rust utility that concatenates and displays directory contents.
///
/// dircat recursively walks through a directory, concatenates the content of discovered
/// files (respecting .gitignore rules and offering extensive filtering), and outputs
/// everything as a single, well-formatted Markdown file. It is designed for speed,
/// developer convenience, and seamless integration with tools that consume Markdown,
/// such as Large Language Models (LLMs) or documentation systems.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the directory/file to process, or a git repository URL to clone.
    #[arg(default_value = ".")]
    pub input_path: String,

    // --- Git Options ---
    #[cfg(feature = "git")]
    /// For git URL inputs, check out a specific branch, tag, or commit instead of the default.
    #[arg(long, alias = "git-ref", value_name = "BRANCH_OR_TAG")]
    pub git_branch: Option<String>,

    #[cfg(feature = "git")]
    /// For git URL inputs, perform a shallow clone with a limited history depth.
    #[arg(long, value_name = "DEPTH")]
    pub git_depth: Option<u32>,

    #[cfg(feature = "git")]
    /// Path to a directory for caching cloned git repositories.
    #[arg(long, value_name = "PATH")]
    pub git_cache_path: Option<String>,

    // --- Filtering Options ---
    /// Maximum file size to include (e.g., "1M", "512k"). Files larger than this are skipped.
    #[arg(short = 'm', long, value_name = "BYTES")]
    pub max_size: Option<String>, // Will be parsed into u64 later

    /// Do not recurse into subdirectories.
    #[arg(short = 'n', long, action = clap::ArgAction::SetTrue)]
    pub no_recursive: bool,

    /// Include only files with these extensions (case-insensitive, repeatable).
    #[arg(short = 'e', long = "ext", value_name = "EXT", num_args = 1..)]
    pub extensions: Option<Vec<String>>,

    /// Exclude files with these extensions (case-insensitive, repeatable).
    #[arg(short = 'x', long = "exclude-ext", value_name = "EXT", num_args = 1..)]
    pub exclude_extensions: Option<Vec<String>>,

    /// Exclude files whose relative path matches any of these regexes (case-insensitive, repeatable).
    #[arg(short = 'X', long = "exclude-regex", value_name = "REGEX", num_args = 1..)]
    pub exclude_path_regex: Option<Vec<String>>,

    /// Ignore files/directories matching these glob patterns (relative to input path, repeatable).
    /// This filter is applied after all other ignore logic (including .gitignore and --last overrides)
    /// and will exclude a file even if it is matched by --last or --only.
    #[arg(short = 'i', long = "ignore", value_name = "GLOB", num_args = 1..)]
    pub ignore_patterns: Option<Vec<String>>,

    /// Include only files whose relative path matches any of these regexes (case-insensitive, repeatable).
    #[arg(short = 'r', long = "regex", value_name = "REGEX", num_args = 1..)]
    pub path_regex: Option<Vec<String>>,

    /// Include only files whose filename (basename) matches any of these regexes (case-insensitive, repeatable).
    #[arg(short = 'd', long = "filename-regex", value_name = "REGEX", num_args = 1..)]
    pub filename_regex: Option<Vec<String>>,

    /// Do not respect .gitignore, .ignore, or other VCS ignore files.
    #[arg(short = 't', long, action = clap::ArgAction::SetTrue)]
    pub no_gitignore: bool,

    /// Include files detected as binary/non-text (default is to skip them).
    #[arg(short = 'B', long, action = clap::ArgAction::SetTrue)]
    pub include_binary: bool,

    /// Skip common lockfiles (e.g., Cargo.lock, package-lock.json).
    #[arg(short = 'K', long, action = clap::ArgAction::SetTrue)]
    pub no_lockfiles: bool,

    // --- Content Processing Options ---
    /// Remove C/C++ style comments (// and /* ... */).
    #[arg(short = 'c', long, action = clap::ArgAction::SetTrue)]
    pub remove_comments: bool,

    /// Remove empty lines (containing only whitespace).
    #[arg(short = 'l', long, action = clap::ArgAction::SetTrue)]
    pub remove_empty_lines: bool,

    // --- Output Formatting Options ---
    /// Only include the filename (basename) in the '## File:' header.
    #[arg(short = 'f', long, action = clap::ArgAction::SetTrue)]
    pub filename_only: bool,

    /// Add line numbers (N | ) to the beginning of each line in the code blocks.
    #[arg(short = 'L', long, action = clap::ArgAction::SetTrue)]
    pub line_numbers: bool,

    /// Wrap filenames in the '## File:' header and summary with backticks (`).
    #[arg(short = 'b', long, action = clap::ArgAction::SetTrue)]
    pub backticks: bool,

    /// Number of backticks to use for the Markdown code fences.
    #[arg(short = 'T', long, value_name = "COUNT", default_value_t = 3)]
    pub ticks: u8,

    // --- Output Destination & Summary ---
    /// Write output to the specified file instead of stdout.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output_file: Option<String>, // Using String, convert to PathBuf later

    #[cfg(feature = "clipboard")]
    /// Copy output to the system clipboard.
    #[arg(short = 'p', long, action = clap::ArgAction::SetTrue)]
    pub paste: bool,

    /// Print a summary list of processed files at the end of the output.
    #[arg(short = 's', long, action = clap::ArgAction::SetTrue)]
    pub summary: bool,

    /// Include line, char (byte), and word counts in the summary (implies -s).
    #[arg(short = 'C', long, action = clap::ArgAction::SetTrue)]
    pub counts: bool,

    // --- Processing Order ---
    /// Process files matching these glob patterns last, in the order specified.
    /// This can override .gitignore rules for the matched files, but will not override
    /// rules from --ignore (-i).
    #[arg(short = 'z', long = "last", value_name = "GLOB", num_args = 1..)]
    pub process_last: Option<Vec<String>>,

    /// Only process files specified with -z/--last. Skip all other files.
    #[arg(short = 'Z', long, action = clap::ArgAction::SetTrue, requires = "process_last")]
    pub only_last: bool,

    /// Shorthand for '--last <GLOB>... --only-last'. Process only files matching these glob patterns.
    /// This can override .gitignore rules for the matched files, but will not override
    /// rules from --ignore (-i).
    #[arg(short = 'O', long = "only", value_name = "GLOB", num_args = 1.., conflicts_with_all = &["process_last", "only_last"])]
    pub only: Option<Vec<String>>,

    // --- Execution Control ---
    /// Perform a dry run. Print files that would be processed but not their content.
    #[arg(short = 'D', long, action = clap::ArgAction::SetTrue)]
    pub dry_run: bool,
}
