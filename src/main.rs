// src/main.rs

use anyhow::Result;
use clap::Parser;
use dircat::cli::Cli;
use dircat::cli::Commands;
use dircat::config::ConfigBuilder;
use dircat::errors::Error;
#[cfg(feature = "progress")]
use dircat::progress::IndicatifProgress;
use dircat::progress::ProgressReporter;
use dircat::run;
use dircat::signal::setup_signal_handler;
use std::sync::Arc;

#[cfg(feature = "web")]
use dircat::web;

// Wrapper struct to handle subcommands without breaking the library's Cli struct
#[derive(Parser)]
struct AppArgs {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    cli: Cli,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // --- Setup ---
    let args = AppArgs::parse();

    // --- Handle Subcommands (Web Server) ---
    #[cfg(feature = "web")]
    if let Some(Commands::Serve { port, no_open }) = &args.command {
        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(web::start_server(*port, !*no_open));
    }

    // Decide whether to show a progress bar. Show it if stderr is a TTY.
    let progress_reporter: Option<Arc<dyn ProgressReporter>> = {
        #[cfg(feature = "progress")]
        {
            if atty::is(atty::Stream::Stderr) {
                Some(Arc::new(IndicatifProgress::new()))
            } else {
                None
            }
        }
        #[cfg(not(feature = "progress"))]
        {
            None
        }
    };

    // --- Configuration & Execution ---
    let config = ConfigBuilder::from_cli(args.cli).build()?;

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
