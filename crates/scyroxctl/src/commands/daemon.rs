//! Daemon management commands.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::cli::{DaemonAction, DaemonCommand};
use crate::client::DaemonClient;

/// Default socket name.
const SOCKET_NAME: &str = "scyroxd.sock";

pub async fn run(cmd: &DaemonCommand) -> Result<()> {
    match &cmd.action {
        DaemonAction::Start { foreground } => start_daemon(*foreground).await,
        DaemonAction::Stop => stop_daemon().await,
        DaemonAction::Status => show_status().await,
        DaemonAction::Restart => restart_daemon().await,
    }
}

async fn start_daemon(foreground: bool) -> Result<()> {
    // Check if already running
    if DaemonClient::connect().await.is_ok() {
        println!("Daemon is already running");
        return Ok(());
    }

    if foreground {
        // Run in foreground - just exec scyroxd
        println!("Starting daemon in foreground...");
        let status = Command::new("scyroxd")
            .status()
            .context("Failed to start scyroxd")?;

        if !status.success() {
            anyhow::bail!("scyroxd exited with status: {}", status);
        }
    } else {
        // Daemonize
        println!("Starting daemon...");

        // Use setsid to create a new session
        let child = Command::new("setsid")
            .args(["--fork", "scyroxd"])
            .spawn()
            .or_else(|_| {
                // Fallback: just spawn in background
                Command::new("scyroxd").spawn()
            })
            .context("Failed to start scyroxd")?;

        println!("Started daemon (PID: {})", child.id());

        // Wait a moment and verify it's running
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        if DaemonClient::connect().await.is_ok() {
            println!("Daemon is now running");
        } else {
            println!("Warning: Could not verify daemon is running");
        }
    }

    Ok(())
}

async fn stop_daemon() -> Result<()> {
    match DaemonClient::connect().await {
        Ok(client) => {
            println!("Stopping daemon...");

            // Send shutdown command
            let _ = client.shutdown().await;

            // Wait for socket to disappear
            let socket_path = get_socket_path()?;
            for _ in 0..10 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if !socket_path.exists() {
                    break;
                }
            }

            println!("Daemon stopped");
            Ok(())
        }
        Err(_) => {
            println!("Daemon is not running");
            Ok(())
        }
    }
}

async fn show_status() -> Result<()> {
    match DaemonClient::connect().await {
        Ok(client) => {
            let info = client.get_info().await?;

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
        Err(_) => {
            println!("Daemon Status: Not running");
        }
    }

    Ok(())
}

async fn restart_daemon() -> Result<()> {
    stop_daemon().await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    start_daemon(false).await
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
