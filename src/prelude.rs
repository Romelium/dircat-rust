//! The `dircat` prelude for convenient library usage.
//!
//! This module re-exports the most commonly used types, traits, and functions
//! from the `dircat` library. By importing everything from this prelude, you can
//! easily get started with using `dircat` programmatically.
//!
//! # Example
//!
//! ```no_run
//! use dircat::prelude::*;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//! // Now you can use ConfigBuilder, CancellationToken, etc. without full paths.
//! let config = ConfigBuilder::new().input_path(".").build()?;
//! let token = CancellationToken::new();
//! let result = execute(&config, &token, None)?;
//!
//! # Ok(())
//! # }
//! ```

pub use crate::cancellation::CancellationToken;
pub use crate::config::{Config, ConfigBuilder, OutputDestination};
pub use crate::core_types::{FileCounts, FileInfo};
pub use crate::errors::{Error, Result};
pub use crate::output::{MarkdownFormatter, OutputFormatter};
pub use crate::processing::filters::{
    remove_comments, remove_empty_lines, ContentFilter, RemoveCommentsFilter,
    RemoveEmptyLinesFilter,
};
pub use crate::{execute, run, DircatResult};

// Also re-export key git utility functions if the feature is enabled.
#[cfg(feature = "git")]
pub use crate::git::{
    download_directory_via_api, get_repo, is_git_url, parse_clone_url, parse_github_folder_url,
    ParsedGitUrl,
};
