//! Scyrox mouse configuration CLI.
//!
//! This CLI provides commands to configure Scyrox mice. It can operate in two modes:
//!
//! - **Client mode** (default): Connects to the scyroxd daemon via IPC
//! - **Direct mode**: Communicates directly with the mouse via USB
//!
//! The CLI automatically detects which mode to use, preferring the daemon if available.

mod cli;
mod commands;
mod output;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use tonic::Status;
use tracing::{debug, error, warn};
use tracing_subscriber::EnvFilter;

use scyrox_client::{Backend, DaemonClient, DirectBackend};

use crate::cli::{Cli, Commands};
use crate::output::Output;

fn main() -> ExitCode {
    if let Err(e) = run() {
        // Extract clean message from tonic Status errors
        if let Some(status) = e.downcast_ref::<Status>() {
            error!("{}", status.message());
        } else {
            error!("{}", e);
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[tokio::main]
async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity level
    let verbosity = if cli.trace { 3 } else { cli.verbose };
    let filter = match verbosity {
        0 => EnvFilter::from_default_env()
            .add_directive("scyroxctl=warn".parse()?)
            .add_directive("scyrox=warn".parse()?),
        1 => EnvFilter::from_default_env()
            .add_directive("scyroxctl=info".parse()?)
            .add_directive("scyrox=info".parse()?),
        2 => EnvFilter::from_default_env()
            .add_directive("scyroxctl=debug".parse()?)
            .add_directive("scyrox=debug".parse()?),
        _ => EnvFilter::from_default_env()
            .add_directive("scyroxctl=trace".parse()?)
            .add_directive("scyrox=trace".parse()?),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Create output handler
    let output = Output::new(cli.format);

    // Handle daemon subcommand separately (doesn't need a backend)
    if let Commands::Daemon(cmd) = &cli.command {
        return commands::daemon::run(cmd, &output).await;
    }

    // Get the appropriate backend
    let backend = get_backend(&cli).await?;

    // Execute the command
    match &cli.command {
        Commands::Get(cmd) => commands::get::run(&*backend, cmd, &output).await,
        Commands::Set(cmd) => commands::set::run(&*backend, cmd, &output).await,
        Commands::Profile(cmd) => commands::profile::run(&*backend, cmd, &output).await,
        Commands::Status => commands::status::run(&*backend, &output).await,
        Commands::Daemon(_) => unreachable!(),
    }
}

/// Get the appropriate backend based on CLI flags and daemon availability.
async fn get_backend(cli: &Cli) -> Result<Box<dyn Backend>> {
    // If --direct flag is set, use direct USB access
    if cli.direct {
        debug!("Using direct USB backend (--direct flag)");
        return Ok(Box::new(DirectBackend::new().await?));
    }

    // Try to connect to daemon first (auto-detect)
    match DaemonClient::connect().await {
        Ok(client) => {
            debug!("Connected to daemon");
            Ok(Box::new(client))
        }
        Err(e) => {
            debug!("Daemon not available: {}", e);
            warn!("Daemon not running, using direct USB access");
            Ok(Box::new(DirectBackend::new().await?))
        }
    }
}
