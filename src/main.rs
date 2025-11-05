// src/main.rs

use anyhow::Result;
use clap::Parser;
use dircat::cli::Cli;
use dircat::config::ConfigBuilder;
use dircat::errors::Error;
use dircat::progress::{IndicatifProgress, ProgressReporter};
use dircat::run;
use dircat::signal::setup_signal_handler;
use std::sync::Arc;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // --- Setup ---
    let cli_args = Cli::parse();

    // Decide whether to show a progress bar. Show it if stderr is a TTY.
    let progress_reporter: Option<std::sync::Arc<dyn ProgressReporter>> =
        if atty::is(atty::Stream::Stderr) {
            Some(Arc::new(IndicatifProgress::new()))
        } else {
            None
        };

    // --- Configuration & Execution ---
    let config = ConfigBuilder::from_cli(cli_args).build()?;
    let token = setup_signal_handler()?;

    let result = run(&config, &token, progress_reporter);

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
