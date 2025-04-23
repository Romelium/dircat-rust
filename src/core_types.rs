use std::path::PathBuf;

/// Represents a file discovered during the walk, potentially with processed content and metadata.
///
/// This struct holds information about each file that passes the initial filtering criteria.
/// It is updated during the processing phase with content and counts.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// The absolute, canonicalized path to the file on the filesystem.
    pub absolute_path: PathBuf,
    /// The path relative to the initial input directory (`config.input_path`).
    /// Used for display purposes in headers and summaries.
    pub relative_path: PathBuf,
    /// The original size of the file in bytes, obtained from metadata.
    pub size: u64,
    /// The processed content of the file as a `String`.
    /// This is `None` initially and populated during the processing phase
    /// after applying filters like comment/empty line removal.
    /// It's `None` if the run is a dry run (`-D`).
    pub processed_content: Option<String>,
    /// Character, word, and line counts calculated from the *original* file content.
    /// This is `None` if counts were not requested (`--counts`) or if the run is a dry run.
    pub counts: Option<FileCounts>,
    /// Flag indicating if this file matched one of the patterns specified
    /// with the `-z`/`--last` argument.
    pub is_process_last: bool,
    /// If `is_process_last` is true, this holds the zero-based index of the
    /// `-z` pattern that this file matched. Used for sorting the "last" files
    /// according to the order they were specified on the command line.
    pub process_last_order: Option<usize>,
}

/// Holds line, character (byte), and word counts for a single file.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileCounts {
    /// Number of lines (newlines `\n`).
    pub lines: usize,
    /// Number of bytes.
    pub characters: usize,
    /// Number of words (separated by whitespace).
    pub words: usize,
}
