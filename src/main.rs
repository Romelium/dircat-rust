// src/main.rs

use anyhow::Result;
use clap::Parser;
use dircat::{
    cli::Cli,
    config::ConfigBuilder,
    discover,
    errors::Error,
    format,
    format_dry_run,
    process,
    progress::{IndicatifProgress, ProgressReporter},
    signal::setup_signal_handler, // Import setup_signal_handler
};
use std::io::Write;
use std::sync::Arc;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // --- Setup ---
    let cli_args = Cli::parse();

    // Decide whether to show a progress bar. Show it if stderr is a TTY.
    let progress_reporter: Option<Arc<dyn ProgressReporter>> = if atty::is(atty::Stream::Stderr) {
        Some(Arc::new(IndicatifProgress::new()))
    } else {
        None
    };

    let config = ConfigBuilder::from_cli(cli_args).build(progress_reporter)?;
    let stop_signal = setup_signal_handler()?;

    // --- Execution ---
    let result = (|| {
        // Discover files based on config
        let discovered_files = discover(&config, stop_signal.clone())?;

        // Set up the output writer (stdout, file, or clipboard buffer)
        let writer_setup = dircat::output::writer::setup_output_writer(&config)?;
        let mut writer: Box<dyn Write + Send> = writer_setup.writer;

        if config.dry_run {
            // Handle Dry Run
            if discovered_files.is_empty() {
                return Err(Error::NoFilesFound);
            }
            format_dry_run(&discovered_files, &config, &mut writer)?;
        } else {
            // Handle Normal Run
            let processed_files = process(discovered_files, &config, stop_signal)?;
            if processed_files.is_empty() {
                return Err(Error::NoFilesFound);
            }
            format(&processed_files, &config, &mut writer)?;
        }

        // Finalize output (e.g., copy to clipboard)
        Ok(dircat::output::writer::finalize_output(
            writer,
            writer_setup.clipboard_buffer,
            &config,
        )?)
    })();

    // --- Error Handling ---
    if let Err(e) = result {
        match e {
            Error::Interrupted => {
                eprintln!("\nOperation cancelled.");
                std::process::exit(130);
            }
            Error::NoFilesFound => {
                eprintln!("dircat: No files found matching the specified criteria.");
                return Ok(());
            }
            _ => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
