//! Defines core data structures used throughout the application pipeline.
//!
//! These structs, `FileInfo` and `FileCounts`, are central to how file data is
//! discovered, processed, and formatted.

use std::path::PathBuf;

/// Represents the raw content of a file, ready for processing.
///
/// This struct is used as input for the `process_content` function, allowing
/// the processing logic to be decoupled from filesystem I/O.
#[derive(Debug, Clone)]
pub struct FileContent {
    /// The path relative to the initial input directory.
    /// This is used for display purposes and for determining the language hint.
    pub relative_path: PathBuf,
    /// The raw byte content of the file.
    pub content: Vec<u8>,
    /// A flag indicating if this file matched one of the patterns specified
    /// with the `--last` argument.
    pub is_process_last: bool,
    /// If `is_process_last` is true, this holds the zero-based index of the `--last`
    /// pattern that this file matched, used for sorting.
    pub process_last_order: Option<usize>,
}

/// Represents a file discovered during the walk, potentially with processed content and metadata.
///
/// This struct holds information about each file that passes the initial filtering criteria.
/// It is updated during the processing phase with content and counts.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// The absolute, canonicalized path to the file on the filesystem.
    pub absolute_path: PathBuf,
    /// The path relative to the initial input directory.
    /// This is used for display purposes in headers and summaries.
    pub relative_path: PathBuf,
    /// The original size of the file in bytes, obtained from metadata.
    pub size: u64,
    /// The processed content of the file as a `String`.
    ///
    /// This is `None` initially and is populated during the processing phase after
    /// applying filters. It remains `None` for a dry run. For binary files, this
    /// will be a lossy UTF-8 representation.
    pub processed_content: Option<String>,
    /// Character, word, and line counts calculated from the file's original content.
    ///
    /// This is `None` if counts were not requested (`--counts`) or for a dry run.
    /// For binary files, only the `characters` (byte) count is meaningful.
    pub counts: Option<FileCounts>,
    /// A flag indicating if this file matched one of the patterns specified
    /// with the `--last` argument.
    pub is_process_last: bool,
    /// If `is_process_last` is true, this holds the zero-based index of the `--last`
    /// pattern that this file matched. This is used for sorting the "last" files
    /// according to the order they were specified.
    pub process_last_order: Option<usize>,
    /// A flag indicating if the file was detected as binary during processing.
    pub is_binary: bool,
}

/// Holds line, character (byte), and word counts for a single file.
///
/// # Examples
///
/// ```
/// use dircat::core_types::FileCounts;
///
/// let counts: FileCounts = Default::default();
/// assert_eq!(counts.lines, 0);
/// assert_eq!(counts.characters, 0);
/// assert_eq!(counts.words, 0);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct FileCounts {
    /// The number of lines (newlines `\n`). This is calculated only for text files.
    pub lines: usize,
    /// The number of bytes.
    pub characters: usize,
    /// The number of words (separated by whitespace). This is calculated only for text files.
    pub words: usize,
}
