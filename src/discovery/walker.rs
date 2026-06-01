use crate::config::path_resolve::ResolvedInput;
use crate::config::DiscoveryConfig;
use anyhow::{Context, Result};
use glob::Pattern;
use ignore::{WalkBuilder, WalkParallel};
use log::debug; // Ensure debug is imported
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::Write;
use tempfile::NamedTempFile;

static WANTS_GIT_RE: Lazy<Regex> = Lazy::new(|| {
    // Matches `.git` specifically as a discrete path component or glob/regex token.
    // Protects against matching `.github`, `.gitignore`, `my_git_file`, etc.
    Regex::new(r"(?i)(?:^|/|\\|\[|\(|\||\^|\s|\*|\?|\+|\{|,)\\?\.git(?:$|/|\\|\]|\)|\||\$|\s|\*|\?|\+|\}|,)").unwrap()
});

/// Configures and builds the `ignore::WalkBuilder` based on `Config`.
pub(super) fn build_walker(
    config: &DiscoveryConfig,
    resolved: &ResolvedInput,
) -> Result<(WalkParallel, Option<NamedTempFile>)> {
    let mut walker_builder = WalkBuilder::new(&resolved.path);
    let mut temp_override_file: Option<NamedTempFile> = None;

    // If --last or --only is used, we can add those patterns as overrides.
    // This will cause the walker to yield matching files even if they are
    // covered by a .gitignore rule, which is the desired behavior.
    if config.use_gitignore {
        walker_builder.standard_filters(true);
        // Explicitly include hidden files (like .github, .env) by default
        walker_builder.hidden(false);
        debug!("Configuring WalkBuilder: standard_filters enabled, hidden files included.");

        if let Some(last_patterns) = &config.process_last {
            // Using OverrideBuilder acts as an inclusion filter, which is not what we want.
            // Instead, we create a temporary, high-precedence ignore file with whitelist
            // rules (`!pattern`) for the --last patterns. This correctly overrides
            // .gitignore rules for just those patterns without filtering out other files.
            let mut file = NamedTempFile::new()
                .with_context(|| "Failed to create temporary override file for --last patterns")?;
            for pattern in last_patterns {
                // Prepend '!' to make it a whitelist pattern.
                writeln!(file, "!{}", pattern)
                    .with_context(|| "Failed to write to temporary override file")?;
            }
            walker_builder.add_custom_ignore_filename(file.path());
            debug!(
                "Added 'process_last' patterns as a custom, high-precedence ignore file: {:?}",
                file.path()
            );
            // Keep the temp file alive until the walker is built and used.
            temp_override_file = Some(file);
        }
    } else {
        // If gitignore is disabled entirely, then disable standard filters.
        walker_builder.standard_filters(false);
        // Ensure hidden files are also explicitly included
        walker_builder.hidden(false);
        debug!("Configuring WalkBuilder: standard_filters disabled (gitignore usage off).");
    }
    // Explicitly disable require_git to ensure .gitignore files are
    // processed even if the test environment doesn't look like a full repo.
    walker_builder.require_git(false);
    debug!("Configuring WalkBuilder: require_git disabled.");

    // The standard_filters(bool) call should handle these implicitly.
    // Explicitly setting them again might interfere or be unnecessary for ignore 0.4+.
    // ---

    if !config.recursive {
        // Max depth 1 means only the immediate children of the base_path
        // If base_path is a file, walkdir handles it correctly (yields just the file)
        walker_builder.max_depth(Some(1));
        debug!("Recursion disabled (max depth: 1).");
    } else {
        debug!("Recursion enabled (no max depth).");
    }

    // --- Determine if .git should be explicitly traversed ---
    // Check if the root path itself contains a ".git" component
    let mut explicitly_wants_git = resolved.path.components().any(|c| c.as_os_str() == ".git");

    if let Some(lasts) = &config.process_last {
        if lasts.iter().any(|p| WANTS_GIT_RE.is_match(p)) {
            explicitly_wants_git = true;
        }
    }
    if let Some(regexes) = &config.path_regex {
        if regexes.iter().any(|r| WANTS_GIT_RE.is_match(r.as_str())) {
            explicitly_wants_git = true;
        }
    }

    // --- Compile custom ignore glob patterns from -i ---
    let mut custom_ignore_globs: Vec<(Pattern, Option<Pattern>)> = Vec::new();
    if let Some(ignore_patterns) = &config.ignore_patterns {
        custom_ignore_globs = ignore_patterns
            .iter()
            .filter_map(|p| match Pattern::new(p) {
                Ok(glob) => {
                    debug!("Compiled custom ignore glob: {}", p);
                    let is_recursive = !p.contains('/') && !p.contains('\\');
                    let recursive_glob = if is_recursive {
                        Pattern::new(&format!("**/{}", p)).ok()
                    } else {
                        None
                    };
                    Some((glob, recursive_glob))
                }
                Err(e) => {
                    log::warn!("Invalid ignore glob pattern '{}': {}", p, e);
                    None
                }
            })
            .collect();
    }

    let has_custom_ignores = !custom_ignore_globs.is_empty();

    // --- Add custom filter entry ---
    // We add the filter entry if we have custom ignores OR if we need to filter out .git
    if has_custom_ignores || !explicitly_wants_git {
        debug!(
            "Adding custom filter_entry (has_custom_ignores: {}, explicitly_wants_git: {})",
            has_custom_ignores, explicitly_wants_git
        );
        let input_path_clone = resolved.path.clone();

        walker_builder.filter_entry(move |entry| {
            let path = entry.path();

            // 1. Skip .git directory by default unless specifically requested
            if !explicitly_wants_git && entry.file_name() == ".git" {
                debug!("Custom filter_entry skipping .git directory: {:?}", path);
                return false;
            }

            // 2. Custom ignore patterns
            if has_custom_ignores {
                if let Ok(relative_path) = path.strip_prefix(&input_path_clone) {
                    if custom_ignore_globs.iter().any(|(glob, rec_glob)| {
                        glob.matches_path(relative_path)
                            || rec_glob
                                .as_ref()
                                .is_some_and(|g| g.matches_path(relative_path))
                    }) {
                        debug!(
                            "Custom filter_entry skipping {:?} matching custom ignore glob",
                            relative_path
                        );
                        return false;
                    }
                }
            }

            true
        });
    } else {
        debug!("No custom ignores and .git is explicitly wanted, skipping filter_entry setup.");
    }

    // Build the walker
    debug!("Building the final walker.");
    Ok((walker_builder.build_parallel(), temp_override_file))
}
