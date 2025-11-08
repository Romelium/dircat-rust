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
pub use extension::passes_extension_filters;
pub use file_type::is_file_type;
pub use lockfile::is_lockfile;
pub use process_last::check_process_last;
pub use size::passes_size_filter;
pub use text_detection::{is_likely_text, is_likely_text_from_buffer};
