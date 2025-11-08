// src/config/builder_logic.rs

use super::ConfigBuilder;
use crate::config::OutputDestination;
use crate::errors::{ConfigError, Result};
use crate::processing::filters::{
    ContentFilter, RemoveCommentsFilter, RemoveEmptyLinesFilter,
};
use std::path::PathBuf;

/// Validates combinations of options on the `ConfigBuilder`.
pub(super) fn validate_builder_options(builder: &ConfigBuilder) -> Result<()> {
    #[cfg(feature = "clipboard")]
    {
        if builder.output_file.is_some() && builder.paste.unwrap_or(false) {
            return Err(ConfigError::Conflict {
                option1: "--output".to_string(),
                option2: "--paste".to_string(),
            }
            .into());
        }
    }
    if builder.ticks.unwrap_or(3) < 3 {
        return Err(ConfigError::InvalidValue {
            option: "--ticks".to_string(),
            reason: "must be 3 or greater".to_string(),
        }
        .into());
    }
    if builder.only_last.unwrap_or(false) && builder.process_last.is_none() {
        return Err(ConfigError::MissingDependency {
            option: "--only-last".to_string(),
            required: "--last".to_string(),
        }
        .into());
    }
    if builder.only.is_some()
        && (builder.process_last.is_some() || builder.only_last.unwrap_or(false))
    {
        return Err(ConfigError::Conflict {
            option1: "--only".to_string(),
            option2: "--last or --only-last".to_string(),
        }
        .into());
    }
    Ok(())
}

/// Constructs the vector of content filters based on builder settings.
pub(super) fn build_content_filters(
    mut content_filters: Vec<Box<dyn ContentFilter>>,
    remove_comments: Option<bool>,
    remove_empty_lines: Option<bool>,
) -> Vec<Box<dyn ContentFilter>> {
    if remove_comments.unwrap_or(false) {
        content_filters.push(Box::new(RemoveCommentsFilter));
    }
    if remove_empty_lines.unwrap_or(false) {
        content_filters.push(Box::new(RemoveEmptyLinesFilter));
    }
    content_filters
}

/// Determines the final `process_last` and `only_last` values, handling the `--only` shorthand.
pub(super) fn determine_process_order(
    only: Option<Vec<String>>,
    process_last: Option<Vec<String>>,
    only_last: Option<bool>,
) -> (Option<Vec<String>>, bool) {
    if let Some(only_patterns) = only {
        (Some(only_patterns), true)
    } else {
        (process_last, only_last.unwrap_or(false))
    }
}

/// Determines the final output destination.
pub(super) fn determine_output_destination(
    output_file: Option<String>,
    #[cfg(feature = "clipboard")] paste: Option<bool>,
) -> OutputDestination {
    if let Some(file_path_str) = output_file {
        OutputDestination::File(PathBuf::from(file_path_str))
    } else {
        #[cfg(feature = "clipboard")]
        if paste.unwrap_or(false) {
            OutputDestination::Clipboard
        } else {
            OutputDestination::Stdout
        }
        #[cfg(not(feature = "clipboard"))]
        {
            OutputDestination::Stdout
        }
    }
}
