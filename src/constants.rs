// src/constants.rs

/// A global header prepended to the start of the generated output.
pub const OUTPUT_FILE_HEADER: &str = "";

/// The separator used before the summary section in the output.
pub const SUMMARY_SEPARATOR: &str = "---";

/// The prefix for the summary header line, which is followed by the file count.
pub const SUMMARY_HEADER_PREFIX: &str = "Processed Files";

/// The default minimum width for formatting line numbers. The actual width adjusts dynamically.
pub const DEFAULT_LINE_NUMBER_WIDTH: usize = 5;

// Add other constants as needed, e.g., default buffer sizes, etc.
