use crate::config::Config;
use anyhow::Result;
use glob::Pattern;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use log::debug; // Ensure debug is imported

/// Configures and builds the `ignore::WalkBuilder` based on `Config`.
pub(super) fn build_walker(config: &Config) -> Result<ignore::Walk> {
    let mut walker_builder = WalkBuilder::new(&config.input_path);

    // If --last or --only is used, we can add those patterns as overrides.
    // This will cause the walker to yield matching files even if they are
    // covered by a .gitignore rule, which is the desired behavior.
    if config.use_gitignore {
        walker_builder.standard_filters(true);
        debug!("Configuring WalkBuilder: standard_filters enabled.");

        if let Some(last_patterns) = &config.process_last {
            let mut ov_builder = OverrideBuilder::new(&config.input_path);
            // Add a general "whitelist everything" pattern first. This has the
            // lowest precedence and defeats the `ignore` crate's default
            // behavior of treating an override as a global whitelist.
            // Now, all non-gitignored files will be yielded, as expected.
            ov_builder.add("**")?;
            for pattern in last_patterns {
                // Now add the user's patterns. These have higher precedence
                // and will successfully override any gitignore rules.
                ov_builder.add(pattern)?;
            }
            let overrides = ov_builder.build()?;
            walker_builder.overrides(overrides);
            debug!("Added 'process_last' patterns as overrides to walker.");
        }
    } else {
        // If gitignore is disabled entirely, then disable standard filters.
        walker_builder.standard_filters(false);
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

    // --- Add custom filter ONLY if custom ignore patterns are provided ---
    if let Some(ignore_patterns) = &config.ignore_patterns {
        // Compile custom ignore glob patterns from -i
        let custom_ignore_globs: Vec<Pattern> = ignore_patterns
            .iter()
            .filter_map(|p| match Pattern::new(p) {
                Ok(glob) => {
                    debug!("Compiled custom ignore glob: {}", p);
                    Some(glob)
                }
                Err(e) => {
                    log::warn!("Invalid ignore glob pattern '{}': {}", p, e);
                    None // Skip invalid patterns
                }
            })
            .collect();

        // Only add the filter if there are valid compiled globs
        if !custom_ignore_globs.is_empty() {
            debug!(
                "Adding custom filter_entry for {} patterns.",
                custom_ignore_globs.len()
            );
            // Clone data needed by the closure
            let input_path_clone = config.input_path.clone();

            walker_builder.filter_entry(move |entry| {
                // This closure only runs if standard filters passed the entry.
                // We only need to check our custom -i patterns.
                let path = entry.path();
                // Persistent Debug Log: Log entry being checked by custom filter
                debug!("Custom filter_entry checking path: {:?}", path);

                // Match globs against the path relative to the *input* path
                if let Ok(relative_path) = path.strip_prefix(&input_path_clone) {
                    if custom_ignore_globs
                        .iter()
                        .any(|glob| glob.matches_path(relative_path))
                    {
                        // Persistent Debug Log: Log skip reason
                        debug!(
                            "Custom filter_entry skipping {:?} matching custom ignore glob (relative path: {:?})",
                            path, relative_path
                        );
                        return false; // Skip this entry due to custom pattern
                    }
                } else {
                    // Fallback: Match against full path if stripping fails
                    // Persistent Debug Log: Log fallback attempt
                    debug!(
                        "Custom filter_entry: Failed to strip prefix, matching against full path: {:?}", path
                    );
                    if custom_ignore_globs.iter().any(|glob| glob.matches_path(path)) {
                        // Persistent Debug Log: Log skip reason (fallback)
                        debug!(
                            "Custom filter_entry skipping {:?} matching custom ignore glob (full path fallback)",
                            path
                        );
                        return false; // Skip this entry due to custom pattern
                    }
                }

                // If not skipped by custom ignore globs, keep the entry
                // Persistent Debug Log: Log keep reason
                debug!("Custom filter_entry keeping path: {:?}", path);
                true
            });
        } else {
            debug!("No valid custom ignore patterns compiled, skipping filter_entry setup.");
        }
    } else {
        debug!("No custom ignore patterns provided (-i), skipping filter_entry setup.");
    }

    // Build the walker
    debug!("Building the final walker.");
    Ok(walker_builder.build())
}
