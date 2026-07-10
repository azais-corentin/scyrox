//! Output formatting for CLI commands.
//!
//! This module provides a unified interface for outputting command results
//! in different formats (text, JSON, etc.).

use scyrox::{BatteryStatus, FirmwareInfo, MouseConfig};
use serde::Serialize;

use crate::cli::OutputFormat;
use scyrox_client::{DaemonConfig, DaemonInfo, ProfileInfo};

/// Print the common configuration fields with the given indent.
fn print_config_fields(config: &MouseConfig, indent: &str) {
    println!("{indent}Polling Rate:      {}", config.polling_rate);
    println!("{indent}Lift-Off Distance: {}", config.lift_off_distance);
    println!(
        "{indent}Sleep Timeout:     {} seconds",
        config.sleep_timeout_seconds
    );
    println!(
        "{indent}Angle Snapping:    {}",
        if config.angle_snapping { "On" } else { "Off" }
    );
    println!(
        "{indent}Ripple Control:    {}",
        if config.ripple_control { "On" } else { "Off" }
    );
    println!(
        "{indent}High Speed Mode:   {}",
        if config.high_speed_mode { "On" } else { "Off" }
    );
    println!(
        "{indent}Long Distance:     {}",
        if config.long_distance_mode {
            "On"
        } else {
            "Off"
        }
    );
}

/// Output handler for formatting command results.
#[derive(Debug, Clone)]
pub struct Output {
    format: OutputFormat,
}

impl Output {
    /// Create a new output handler with the specified format.
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Get the current output format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Print mouse configuration.
    pub fn print_config(&self, config: &MouseConfig) {
        match self.format {
            OutputFormat::Text => {
                println!("Configuration:");
                print_config_fields(config, "  ");
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(config).unwrap_or_default());
            }
        }
    }

    /// Print battery status.
    pub fn print_battery(&self, battery: &BatteryStatus) {
        match self.format {
            OutputFormat::Text => {
                println!("Battery:");
                println!("  Voltage:    {} mV", battery.voltage_mv);
                println!("  Percentage: {}%", battery.percentage);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(battery).unwrap_or_default());
            }
        }
    }

    /// Print firmware information.
    pub fn print_firmware(&self, firmware: &FirmwareInfo) {
        match self.format {
            OutputFormat::Text => {
                println!("Firmware:");
                println!("  Mouse:    {}", firmware.mouse_version);
                if let Some(receiver) = &firmware.receiver_version {
                    println!("  Receiver: {}", receiver);
                }
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(firmware).unwrap_or_default());
            }
        }
    }

    /// Print a single labeled value.
    pub fn print_value<T: Serialize + std::fmt::Display>(&self, value: &T) {
        match self.format {
            OutputFormat::Text => {
                println!("{}", value);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(value).unwrap_or_default());
            }
        }
    }

    /// Print device and daemon status.
    pub fn print_status(&self, status: &StatusOutput) {
        match self.format {
            OutputFormat::Text => {
                println!("Device Status:");
                println!(
                    "  Connected: {}",
                    if status.connected { "Yes" } else { "No" }
                );

                if status.connected {
                    if let Some(ref polling_rate) = status.polling_rate {
                        println!("  Polling Rate: {}", polling_rate);
                    }
                    if let Some(ref battery) = status.battery {
                        println!(
                            "  Battery: {}% ({} mV)",
                            battery.percentage, battery.voltage_mv
                        );
                    }
                }

                println!();
                println!("Daemon Status:");
                match &status.daemon {
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
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(status).unwrap_or_default());
            }
        }
    }

    /// Print a list of profiles.
    pub fn print_profiles(&self, profiles: &[ProfileInfo]) {
        match self.format {
            OutputFormat::Text => {
                if profiles.is_empty() {
                    println!("No profiles found.");
                    println!("Create one with: scyroxctl profile create <name>");
                    return;
                }

                println!("Profiles:");
                for profile in profiles {
                    let default_marker = if profile.is_default { " (default)" } else { "" };
                    println!("  {} - {}{}", profile.id, profile.name, default_marker);
                }
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(profiles).unwrap_or_default());
            }
        }
    }

    /// Print a single profile.
    pub fn print_profile(&self, profile: &ProfileInfo) {
        match self.format {
            OutputFormat::Text => {
                println!("Profile: {}", profile.name);
                println!("  ID:      {}", profile.id);
                println!(
                    "  Default: {}",
                    if profile.is_default { "Yes" } else { "No" }
                );
                println!();
                println!("Configuration:");
                print_config_fields(&profile.config, "  ");
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(profile).unwrap_or_default());
            }
        }
    }

    /// Print daemon configuration.
    pub fn print_daemon_config(&self, config: &DaemonConfig) {
        match self.format {
            OutputFormat::Text => {
                println!("Daemon Configuration:");
                println!("  Low Battery Threshold: {}%", config.low_battery_threshold);
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(config).unwrap_or_default());
            }
        }
    }

    /// Print a success message.
    pub fn print_success(&self, message: &str) {
        match self.format {
            OutputFormat::Text => {
                println!("{}", message);
            }
            OutputFormat::Json => {
                let output = SuccessOutput {
                    success: true,
                    message: message.to_string(),
                };
                println!("{}", serde_json::to_string(&output).unwrap_or_default());
            }
        }
    }
}

/// Status output structure for JSON serialization.
#[derive(Debug, Clone, Serialize)]
pub struct StatusOutput {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polling_rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<BatteryStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daemon: Option<DaemonInfo>,
}

/// Success output structure for JSON serialization.
#[derive(Debug, Clone, Serialize)]
pub struct SuccessOutput {
    pub success: bool,
    pub message: String,
}
