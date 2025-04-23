// Declare modules that will contain the application's core logic
pub mod cli;
pub mod config;
pub mod constants;
pub mod core_types;
pub mod discovery;
pub mod errors;
// filtering module is now correctly declared and contains submodules
mod filtering; // <-- Keep this declaration
pub mod output;
pub mod processing;
pub mod signal;

use crate::config::Config;
use crate::core_types::FileInfo;
use anyhow::Result;
use log::debug;
use log::info;
use std::io::Write; // Import Write trait
use std::sync::atomic::{AtomicBool, Ordering}; // Keep this top-level import
use std::sync::Arc; // Add Arc to the top-level imports

/// The main entry point for the dircat library logic.
///
/// This function orchestrates the entire process:
/// 1. Sets up signal handling for graceful shutdown (Ctrl+C).
/// 2. Discovers files based on the provided configuration and filters.
/// 3. Sets up the appropriate output writer (stdout, file, or clipboard buffer).
/// 4. Handles dry runs by printing the list of files that would be processed.
/// 5. For normal runs, processes the discovered files in parallel:
///    - Reads file content.
///    - Calculates counts (lines, chars, words) if requested.
///    - Applies content filters (comment removal, empty line removal) if requested.
/// 6. Generates the final concatenated output, including headers, formatted file blocks, and an optional summary.
/// 7. Finalizes the output (e.g., copying to clipboard if requested).
///
/// # Arguments
///
/// * `config` - The `Config` struct derived from command-line arguments, containing all settings for the run.
///
/// # Errors
///
/// Returns `anyhow::Error` if any step fails, such as:
/// * Signal handler setup failure.
/// * I/O errors during file discovery or reading.
/// * Errors during output writing or finalization (e.g., clipboard access).
/// * `AppError::Interrupted` if the process is cancelled via Ctrl+C.
///
/// # Returns
///
/// Returns `Ok(())` on successful completion.
pub fn run(config: Config) -> Result<()> {
    info!("Starting dircat run with config: {:?}", config);

    // 1. Set up signal handling (Ctrl+C) - Conditionally compiled
    #[cfg(not(test))]
    let stop_signal: Arc<AtomicBool> = {
        // Type annotation optional but can be helpful
        info!("Setting up real signal handler.");
        signal::setup_signal_handler()?
    };
    #[cfg(test)]
    let stop_signal: Arc<AtomicBool> = {
        // Type annotation optional but can be helpful
        // In tests, use a dummy signal that always stays true.
        info!("Using dummy signal handler for test run.");
        // Arc and AtomicBool are now available from the top-level imports
        Arc::new(AtomicBool::new(true))
    };
    // Persistent Debug Log: Log the state right after setup/dummy creation
    debug!(
        "stop_signal initial state: {}",
        stop_signal.load(Ordering::Relaxed) // Ordering is available from top-level import
    );
    info!("Signal handler setup complete (real or dummy).");

    // 2. Discover files based on config
    let (mut normal_files, mut last_files) =
        discovery::discover_files(&config, stop_signal.clone())?; // Arc::clone works
    info!(
        "Discovered {} normal files and {} files to process last.",
        normal_files.len(),
        last_files.len()
    );

    // Exit early if no files are found or if doing a dry run (dry run handled in output)
    if normal_files.is_empty() && last_files.is_empty() && !config.dry_run {
        info!("No files found matching criteria. Exiting.");
        // Print a message to stderr for clarity
        eprintln!("dircat: No files found matching the specified criteria.");
        return Ok(());
    }

    // 3. Set up the output writer (might be stdout, file, or a buffer for clipboard)
    // We need the writer setup before processing if dry_run is enabled.
    // Use a block to manage the writer's scope for potential buffering/finalization.
    let result = {
        let writer_setup = output::writer::setup_output_writer(&config)?;
        let mut writer: Box<dyn Write + Send> = writer_setup.writer;
        let clipboard_buffer = writer_setup.clipboard_buffer; // Keep buffer handle if needed

        // 4. Handle Dry Run - If dry run, print file list and exit early.
        if config.dry_run {
            info!("Performing dry run.");
            // Combine normal and last files for dry run display
            // Sorting happens within write_dry_run_output
            let all_files_refs: Vec<&FileInfo> =
                normal_files.iter().chain(last_files.iter()).collect();
            output::dry_run::write_dry_run_output(&mut writer, &all_files_refs, &config)?;
            info!("Dry run output generated.");
        } else {
            // 5. Process files (read content, apply filters, count) - only if not dry run
            info!("Starting file content processing...");
            // Process normal and last files separately but using the same logic
            processing::process_batch(&mut normal_files, &config, stop_signal.clone())?; // Arc::clone works
            processing::process_batch(&mut last_files, &config, stop_signal)?; // Arc works
            info!("File content processing complete.");

            // 6. Generate output (headers, content, summary)
            info!("Generating final output...");
            // Pass slices to generate_output
            output::generate_output(&normal_files, &last_files, &config, &mut writer)?;
            info!("Final output generated.");
        }

        // 7. Finalize output (e.g., copy to clipboard if needed)
        // Pass the original writer Box and the optional buffer handle
        output::writer::finalize_output(writer, clipboard_buffer, &config)
    }; // Writer goes out of scope here, flushing/closing files

    match result {
        Ok(_) => info!("dircat run completed successfully."),
        Err(e) => {
            // Log the error before propagating it
            log::error!("dircat run failed: {}", e);
            // Check if it was an interruption error for user-friendly message
            if let Some(app_err) = e.downcast_ref::<errors::AppError>() {
                if matches!(app_err, errors::AppError::Interrupted) {
                    // Persistent Debug Log: Log interruption source if possible (already logged in discovery/processing)
                    log::error!("Run failed due to AppError::Interrupted.");
                    eprintln!("Operation cancelled."); // User-friendly message for interruption
                                                       // Exit with a specific code for interruption (optional)
                                                       // std::process::exit(130); // Standard exit code for SIGINT
                } else {
                    eprintln!("Error: {}", e); // Print other AppErrors to stderr
                }
            } else {
                eprintln!("Error: {}", e); // Print non-AppError errors (e.g., from anyhow contexts)
            }
            return Err(e); // Propagate the error
        }
    }

    Ok(())
}
