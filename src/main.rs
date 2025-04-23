// src/main.rs

use anyhow::Result;
use clap::Parser;
use dircat::{cli::Cli, config::Config, run}; // Use run from lib.rs

fn main() -> Result<()> {
    // Initialize logging (optional, controlled by RUST_LOG env var)
    // Consider making log level configurable via CLI flags later.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Parse command-line arguments
    let cli_args = Cli::parse();

    // Convert CLI arguments into an internal configuration, handling errors
    let config = Config::try_from(cli_args)?;

    // Execute the main application logic
    run(config) // Directly return the Result from run
}
