// src/filtering/mod.rs

// Declare the sub-modules within the filtering module
mod extension;
mod file_type;
mod lockfile; // <-- ADDED
mod process_last;
mod size;
mod text_detection; // <-- ADDED

// Re-export the functions needed by other parts of the crate (like discovery)
// Use pub(crate) to make them accessible within the crate but not outside.
pub(crate) use extension::passes_extension_filters;
pub(crate) use file_type::is_file_type;
pub(crate) use lockfile::is_lockfile; // <-- ADDED export
pub(crate) use process_last::check_process_last;
pub(crate) use size::passes_size_filter;
pub(crate) use text_detection::is_likely_text; // <-- ADDED export
