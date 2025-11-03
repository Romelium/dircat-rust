// src/filtering/mod.rs

// Declare the sub-modules within the filtering module
mod extension;
mod file_type;
mod lockfile;
mod process_last;
mod size;
mod text_detection;

// Re-export the functions needed by other parts of the crate (like discovery)
// Use pub(crate) to make them accessible within the crate but not outside.
pub(crate) use extension::passes_extension_filters;
pub(crate) use file_type::is_file_type;
pub(crate) use lockfile::is_lockfile;
pub(crate) use process_last::check_process_last;
pub(crate) use size::passes_size_filter;
pub use text_detection::{is_likely_text, is_likely_text_from_buffer};
