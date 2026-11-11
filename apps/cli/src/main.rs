//! `heric-cli`: command line interface for Heric.
//!
//! Targets parity with the original `inso` CLI: run collections, run tests,
//! export and import workspaces, lint OpenAPI specs and generate code.
//!
//! The command surface is fixed in Phase 1 so the argument names are reviewed
//! before the ports exist. Every subcommand parses and dispatches today; the
//! ones that need a port report the milestone that implements it instead of
//! calling into a `todo!()`, so the binary is usable and its behaviour is
//! testable from the start.

use anyhow::{Result, bail};
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

    /// Copy an Insomnia installation into Heric's own store.
    Migrate {
        /// The Insomnia data directory. Detected when omitted.
        #[arg(long)]
        from: Option<std::path::PathBuf>,
        /// Report what would move without writing anything.
        #[arg(long)]
        dry_run: bool,
    },

    /// Import a Postman, HAR, cURL or OpenAPI source.
    Import {
        /// The file to read, or `-` for standard input.
        file: String,
        /// Create the documents without asking for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Export a workspace.
    Export {
        /// The workspace id, or the active one when omitted.
        #[arg(long)]
        workspace: Option<String>,
        /// `insomnia-v5`, `insomnia-v4` or `har`.
        #[arg(long, default_value = "insomnia-v5")]
        format: String,
        /// Write plaintext secrets instead of masking them.
        #[arg(long)]
        plaintext: bool,
        /// The destination file. Standard output when omitted.
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },

    /// Send a request and print the response.
    Run {
        /// The request id.
        request: String,
        /// Run it this many times, for iteration data.
        #[arg(long, default_value_t = 1)]
        iterations: u32,
    },

    /// Run a workspace's tests.
    Test {
        /// The workspace id, or the active one when omitted.
        #[arg(long)]
        workspace: Option<String>,
    },

    /// Lint an OpenAPI or Swagger document.
    Lint {
        /// The specification to check.
        file: std::path::PathBuf,
        /// Exit non-zero on warnings as well as errors.
        #[arg(long)]
        strict: bool,
    },

    /// Print a code snippet for a request.
    Snippet {
        /// The request id.
        request: String,
        /// The target language, such as `curl` or `python`.
        #[arg(long, default_value = "curl")]
        target: String,
        /// The HTTP client within that target, such as `requests`.
        #[arg(long)]
        client: Option<String>,
    },
}

/// The milestone that implements a subcommand whose port does not exist yet.
///
/// Reported rather than panicking: a `todo!()` inside the binary would print a
/// Rust backtrace, and the point of the Phase 1 stubs is that the contracts
/// compile and the user-facing behaviour is stated.
fn pending(command: &str, milestone: &str) -> anyhow::Error {
    anyhow::anyhow!("`{command}` is scheduled for {milestone} and is not implemented yet")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Version) => {
            println!("heric-cli {}", env!("CARGO_PKG_VERSION"));
            println!("heric-core {}", heric_core::version());
        }
        None => {
            println!(
                "heric-cli {} — try `heric-cli --help`",
                env!("CARGO_PKG_VERSION")
            );
        }
        Some(Command::Migrate { from, dry_run }) => {
            let _ = (from, dry_run);
            return Err(pending("migrate", "M2"));
        }
        Some(Command::Import { file, yes }) => {
            let _ = (file, yes);
            bail!(pending("import", "M5"));
        }
        Some(Command::Export {
            workspace,
            format,
            plaintext,
            output,
        }) => {
            let _ = (workspace, format, plaintext, output);
            bail!(pending("export", "M6"));
        }
        Some(Command::Run {
            request,
            iterations,
        }) => {
            let _ = (request, iterations);
            bail!(pending("run", "M3"));
        }
        Some(Command::Test { workspace }) => {
            let _ = workspace;
            bail!(pending("test", "M7"));
        }
        Some(Command::Lint { file, strict }) => {
            let _ = (file, strict);
            bail!(pending("lint", "M5"));
        }
        Some(Command::Snippet {
            request,
            target,
            client,
        }) => {
            let _ = (request, target, client);
            bail!(pending("snippet", "M6"));
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

    #[test]
    fn migrate_takes_an_optional_source_and_a_dry_run_flag() {
        let cli = Cli::parse_from([
            "heric-cli",
            "migrate",
            "--from",
            "/tmp/Insomnia",
            "--dry-run",
        ]);
        match cli.command {
            Some(Command::Migrate { from, dry_run }) => {
                assert_eq!(from.as_deref(), Some(std::path::Path::new("/tmp/Insomnia")));
                assert!(dry_run);
            }
            other => panic!("expected migrate, got {other:?}"),
        }
    }

    #[test]
    fn export_defaults_to_the_insomnia_v5_format() {
        let cli = Cli::parse_from(["heric-cli", "export"]);
        match cli.command {
            Some(Command::Export { format, .. }) => assert_eq!(format, "insomnia-v5"),
            other => panic!("expected export, got {other:?}"),
        }
    }

    #[test]
    fn run_takes_a_request_and_defaults_to_one_iteration() {
        let cli = Cli::parse_from(["heric-cli", "run", "req_1"]);
        match cli.command {
            Some(Command::Run {
                request,
                iterations,
            }) => {
                assert_eq!(request, "req_1");
                assert_eq!(iterations, 1);
            }
            other => panic!("expected run, got {other:?}"),
        }
    }
}
