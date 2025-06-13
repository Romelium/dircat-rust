// src/config/validation.rs

use crate::cli::Cli;
use anyhow::{anyhow, Result};

/// Validates combinations of CLI options that clap cannot easily express.
pub(super) fn validate_cli_options(cli: &Cli) -> Result<()> {
    if cli.output_file.is_some() && cli.paste {
        return Err(anyhow!(
            "Cannot use --output <FILE> (-o) and --paste (-p) simultaneously."
        ));
    }

    // Add other cross-argument validations here if needed

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn test_output_conflict_validation() {
        // This specific case might also be caught by clap if `conflicts_with` is used,
        // but this validator serves as a backup or for more complex rules.
        let cli = Cli::parse_from(["dircat", ".", "-o", "out.txt", "-p"]);
        let result = validate_cli_options(&cli);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("simultaneously"));
    }

    #[test]
    fn test_valid_options_pass() -> Result<()> {
        let cli_stdout = Cli::parse_from(["dircat", "."]);
        validate_cli_options(&cli_stdout)?;

        let cli_file = Cli::parse_from(["dircat", ".", "-o", "out.txt"]);
        validate_cli_options(&cli_file)?;

        // Clipboard feature is always enabled now.
        let cli_paste = Cli::parse_from(["dircat", ".", "-p"]);
        validate_cli_options(&cli_paste)?;

        Ok(())
    }
}
