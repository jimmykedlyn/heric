//! `heric-cli`: command line interface for Heric.
//!
//! Targets parity with the original `inso` CLI: run collections, run tests,
//! export and import workspaces, lint OpenAPI specs and generate code.

use clap::{Parser, Subcommand};

/// Command line entry point.
#[derive(Debug, Parser)]
#[command(name = "heric-cli", version, about = "Heric command line interface")]
struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    command: Option<Command>,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the Heric core version.
    Version,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Version) => {
            println!("heric-core {}", heric_core::version());
        }
        None => {
            println!("heric-cli {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_version_subcommand() {
        let cli = Cli::parse_from(["heric-cli", "version"]);
        assert!(matches!(cli.command, Some(Command::Version)));
    }

    #[test]
    fn cli_parses_without_subcommand() {
        let cli = Cli::parse_from(["heric-cli"]);
        assert!(cli.command.is_none());
    }
}
