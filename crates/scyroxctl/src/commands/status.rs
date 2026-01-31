//! Status command.

use anyhow::Result;

use crate::backend::Backend;

pub async fn run(backend: &dyn Backend) -> Result<()> {
    // Device status
    let connected = backend.is_connected().await;
    println!("Device Status:");
    println!("  Connected: {}", if connected { "Yes" } else { "No" });

    if connected {
        // Show current config
        if let Ok(config) = backend.get_config().await {
            println!("  Polling Rate: {}", config.polling_rate);
        }

        // Show battery
        if let Ok(battery) = backend.get_battery().await {
            println!(
                "  Battery: {}% ({} mV)",
                battery.percentage, battery.voltage_mv
            );
        }
    }

    // Daemon status (if available)
    println!();
    println!("Daemon Status:");
    match backend.get_daemon_info().await? {
        Some(info) => {
            println!("  Running: Yes");
            println!("  Version: {}", info.version);
            println!("  Uptime:  {} seconds", info.uptime_seconds);
            println!(
                "  Device:  {}",
                if info.connected {
                    "Connected"
                } else {
                    "Disconnected"
                }
            );
        }
        None => {
            println!("  Running: No (using direct USB access)");
        }
    }

    Ok(())
}
