// src/main.rs

use anyhow::Result;
use clap::Parser;
use dircat::{
    cli::Cli,
    config::ConfigBuilder,
    errors::Error,
    progress::{IndicatifProgress, ProgressReporter},
    run,
    signal::setup_signal_handler,
};
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
    let result = run(&config, stop_signal);

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
