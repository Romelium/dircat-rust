// src/filtering/mod.rs

//! Provides standalone functions for file filtering logic.
//!
//! These functions are used by the discovery stage to determine which files
//! should be included in the processing pipeline. They are exposed publicly
//! to allow for their use in other contexts.

// Declare the sub-modules within the filtering module
mod extension;
mod file_type;
mod lockfile;
mod process_last;
mod size;
mod text_detection;

// Re-export the functions needed by other parts of the crate (like discovery)
// Use pub to make them accessible within the crate and as part of the public library API.
pub use extension::passes_extension_filters;
pub use file_type::is_file_type;
pub use lockfile::is_lockfile;
pub use process_last::check_process_last;
pub use size::passes_size_filter;
pub use text_detection::{is_likely_text, is_likely_text_from_buffer};
