//! Scyrox mouse configuration CLI.
//!
//! This CLI provides commands to configure Scyrox mice. It can operate in two modes:
//!
//! - **Client mode** (default): Connects to the scyroxd daemon via IPC
//! - **Direct mode**: Communicates directly with the mouse via USB
//!
//! The CLI automatically detects which mode to use, preferring the daemon if available.

mod backend;
mod cli;
mod client;
mod commands;
mod direct;

use anyhow::Result;
use clap::Parser;
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::backend::Backend;
use crate::cli::{Cli, Commands};
use crate::client::DaemonClient;
use crate::direct::DirectBackend;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity level
let verbosity = if cli.trace { 2 } else { cli.verbose };
    if verbosity > 0 {
        let filter = match verbosity {
            1 => EnvFilter::from_default_env()
                .add_directive("scyroxctl=debug".parse()?)
                .add_directive("scyrox=debug".parse()?),
            _ => EnvFilter::from_default_env()
                .add_directive("scyroxctl=trace".parse()?)
                .add_directive("scyrox=trace".parse()?),
        };
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .init();
    }

    // Handle daemon subcommand separately (doesn't need a backend)
    if let Commands::Daemon(cmd) = &cli.command {
        return commands::daemon::run(cmd).await;
    }

    // Get the appropriate backend
    let backend = get_backend(&cli).await?;

    // Execute the command
    match &cli.command {
        Commands::Get(cmd) => commands::get::run(&*backend, cmd).await,
        Commands::Set(cmd) => commands::set::run(&*backend, cmd).await,
        Commands::Profile(cmd) => commands::profile::run(&*backend, cmd).await,
        Commands::Status => commands::status::run(&*backend).await,
        Commands::Daemon(_) => unreachable!(),
    }
}

/// Get the appropriate backend based on CLI flags and daemon availability.
async fn get_backend(cli: &Cli) -> Result<Box<dyn Backend>> {
    // If --direct flag is set, use direct USB access
    if cli.direct {
        debug!("Using direct USB backend (--direct flag)");
        return Ok(Box::new(DirectBackend::new()?));
    }

    // Try to connect to daemon first (auto-detect)
    match DaemonClient::connect().await {
        Ok(client) => {
            debug!("Connected to daemon");
            Ok(Box::new(client))
        }
        Err(e) => {
            debug!("Daemon not available: {}", e);
            eprintln!("Note: Daemon not running, using direct USB access");
            Ok(Box::new(DirectBackend::new()?))
        }
    }
}
