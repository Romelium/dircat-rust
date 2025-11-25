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
    // Initialize logging. Default to 'info' if RUST_LOG is not set.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(
                if cfg!(debug_assertions) {
                    "dircat=debug".parse().unwrap()
                } else {
                    "dircat=info".parse().unwrap()
                },
            ),
        )
        .init();

    log::info!("Starting dircat v{}...", env!("CARGO_PKG_VERSION"));
    log::debug!("Raw arguments: {:?}", std::env::args().collect::<Vec<_>>());

    // SECURITY: Panic Hook to prevent info leaks
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => "Box<Any>",
        };
        // Simple redaction for panic messages
        eprintln!(
            "Application Error: {}",
            msg.replace(env!("CARGO_MANIFEST_DIR"), "<redacted>")
                .replace(std::path::MAIN_SEPARATOR, "/")
        );
    }));

    // --- Setup ---
    let args = AppArgs::parse();

    // --- Handle Subcommands (Web Server) ---
    #[cfg(feature = "web")]
    if let Some(Commands::Serve {
        port,
        no_open,
        safe,
        allowed_domains,
        safe_max_file_size,
        safe_max_repo_size,
        safe_timeout,
    }) = &args.command
    {
        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(web::start_server(
            *port,
            !*no_open,
            *safe,
            allowed_domains.clone(),
            safe_max_file_size.clone(),
            safe_max_repo_size.clone(),
            *safe_timeout,
        ));
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
    log::debug!("Configuration built successfully.");

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
