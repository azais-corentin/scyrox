//! Daemon management commands.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Serialize;

use crate::cli::{DaemonAction, DaemonCommand, OutputFormat};
use crate::client::DaemonClient;
use crate::output::Output;

/// Default socket name.
const SOCKET_NAME: &str = "scyroxd.sock";

pub async fn run(cmd: &DaemonCommand, output: &Output) -> Result<()> {
    match &cmd.action {
        DaemonAction::Start { foreground } => start_daemon(*foreground, output).await,
        DaemonAction::Stop => stop_daemon(output).await,
        DaemonAction::Status => show_status(output).await,
        DaemonAction::Restart => restart_daemon(output).await,
    }
}

async fn start_daemon(foreground: bool, output: &Output) -> Result<()> {
    if DaemonClient::connect().await.is_ok() {
        output.print_success("Daemon is already running");
        return Ok(());
    }

    if foreground {
        output.print_success("Starting daemon in foreground...");
        let status = Command::new("scyroxd")
            .status()
            .context("Failed to start scyroxd")?;

        if !status.success() {
            anyhow::bail!("scyroxd exited with status: {}", status);
        }
    } else {
        output.print_success("Starting daemon...");

        let child = Command::new("setsid")
            .args(["--fork", "scyroxd"])
            .spawn()
            .or_else(|_| Command::new("scyroxd").spawn())
            .context("Failed to start scyroxd")?;

        output.print_success(&format!("Started daemon (PID: {})", child.id()));
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        if DaemonClient::connect().await.is_ok() {
            output.print_success("Daemon is now running");
        } else {
            output.print_success("Warning: Could not verify daemon is running");
        }
    }

    Ok(())
}

async fn stop_daemon(output: &Output) -> Result<()> {
    match DaemonClient::connect().await {
        Ok(client) => {
            output.print_success("Stopping daemon...");
            let _ = client.shutdown().await;
            let socket_path = get_socket_path()?;
            for _ in 0..10 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if !socket_path.exists() {
                    break;
                }
            }

            output.print_success("Daemon stopped");
            Ok(())
        }
        Err(_) => {
            output.print_success("Daemon is not running");
            Ok(())
        }
    }
}

/// Daemon status output for JSON serialization.
#[derive(Debug, Clone, Serialize)]
struct DaemonStatusOutput {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_connected: Option<bool>,
}

async fn show_status(output: &Output) -> Result<()> {
    match DaemonClient::connect().await {
        Ok(client) => {
            let info = client.get_info().await?;

            let status = DaemonStatusOutput {
                running: true,
                version: Some(info.version.clone()),
                uptime_seconds: Some(info.uptime_seconds),
                device_connected: info.device_status.map(|d| d.connected),
            };

            match output.format() {
                OutputFormat::Text => {
                    println!("Daemon Status: Running");
                    println!("  Version: {}", info.version);
                    println!("  Uptime:  {} seconds", info.uptime_seconds);

                    if let Some(device) = info.device_status {
                        println!(
                            "  Device:  {}",
                            if device.connected {
                                "Connected"
                            } else {
                                "Disconnected"
                            }
                        );
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&status).unwrap_or_default());
                }
            }
        }
        Err(_) => {
            let status = DaemonStatusOutput {
                running: false,
                version: None,
                uptime_seconds: None,
                device_connected: None,
            };

            match output.format() {
                OutputFormat::Text => {
                    println!("Daemon Status: Not running");
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&status).unwrap_or_default());
                }
            }
        }
    }

    Ok(())
}

async fn restart_daemon(output: &Output) -> Result<()> {
    stop_daemon(output).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    start_daemon(false, output).await
}

/// Get the socket path.
fn get_socket_path() -> Result<PathBuf> {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime_dir).join("scyrox").join(SOCKET_NAME));
    }

    let dirs =
        ProjectDirs::from("", "", "scyrox").context("Failed to determine project directories")?;
    let state_dir = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
    Ok(state_dir.join(SOCKET_NAME))
}
