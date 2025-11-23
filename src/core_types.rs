//! Defines core data structures used throughout the application pipeline.
//!
//! These structs, `FileInfo` and `FileCounts`, are central to how file data is
//! discovered, processed, and formatted.

use std::path::PathBuf;

/// Represents the raw content of a file, ready for processing.
///
/// This struct is used as input for the `process_content` function, allowing
/// the processing logic to be decoupled from filesystem I/O.
///
/// # Examples
///
/// ```
/// use dircat::core_types::FileContent;
/// use std::path::PathBuf;
///
/// let file_content = FileContent {
///     relative_path: PathBuf::from("src/main.rs"),
///     content: b"fn main() {}".to_vec(),
///     is_process_last: false,
///     process_last_order: None,
///     ..Default::default()
/// };
///
/// assert_eq!(file_content.relative_path.to_str(), Some("src/main.rs"));
/// ```
#[derive(Debug, Clone, Default)]
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
///
/// # Examples
///
/// ```
/// use dircat::core_types::{FileInfo, FileCounts};
/// use std::path::PathBuf;
///
/// let file_info = FileInfo {
///     absolute_path: PathBuf::from("/path/to/project/src/main.rs"),
///     relative_path: PathBuf::from("src/main.rs"),
///     size: 123,
///     processed_content: Some("fn main() {}".to_string()),
///     counts: Some(FileCounts { lines: 1, characters: 12, words: 2 }),
///     is_process_last: false,
///     process_last_order: None,
///     is_binary: false,
///     ..Default::default()
/// };
///
/// assert_eq!(file_info.size, 123);
/// assert!(file_info.processed_content.is_some());
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
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
/// This struct is populated during the processing stage if the `--counts` flag is used.
/// For text files, all fields are calculated. For binary files, only the `characters`
/// (byte count) field is meaningful.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FileCounts {
    /// The number of lines, calculated by counting newline (`\n`) characters.
    pub lines: usize,
    /// The number of bytes in the file's original content.
    pub characters: usize,
    /// The number of words, calculated by splitting the content by whitespace.
    pub words: usize,
}
