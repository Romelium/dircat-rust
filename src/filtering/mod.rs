// src/filtering/mod.rs

// This module contains functions that implement specific filtering logic
// based on the application's configuration. They are typically called
// during the file discovery phase.

// Declare the modules containing the filtering functions
mod extension;
mod file_type;
mod process_last;
// Removed: mod regex;
mod size;

// Make the functions available within the crate (used by discovery module)
pub(crate) use extension::passes_extension_filters;
pub(crate) use file_type::is_file_type;
pub(crate) use process_last::check_process_last;
// Removed: pub(crate) use regex::passes_regex_filters;
pub(crate) use size::passes_size_filter;

// Removed unused pub(crate) use statements as they are not needed for re-exporting
// pub(crate) use extension::passes_extension_filters; // Now used directly
// pub(crate) use file_type::is_file_type; // Now used directly
// pub(crate) use process_last::check_process_last; // Now used directly
// pub(crate) use size::passes_size_filter; // Now used directly
