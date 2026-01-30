//! Daemon configuration management.

use std::path::Path;

use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info};

/// Daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Battery percentage threshold for low battery alerts.
    #[serde(default = "default_low_battery_threshold")]
    pub low_battery_threshold: u8,

    /// Polling interval for battery status in seconds.
    #[serde(default = "default_battery_poll_interval")]
    pub battery_poll_interval_secs: u64,

    /// Whether to auto-apply the default profile on device connection.
    #[serde(default = "default_auto_apply")]
    pub auto_apply_on_connect: bool,

    /// Default profile ID to apply on connection (if auto_apply_on_connect is true).
    #[serde(default)]
    pub default_profile_id: Option<String>,
}

fn default_low_battery_threshold() -> u8 {
    20
}

fn default_battery_poll_interval() -> u64 {
    60
}

fn default_auto_apply() -> bool {
    true
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            low_battery_threshold: default_low_battery_threshold(),
            battery_poll_interval_secs: default_battery_poll_interval(),
            auto_apply_on_connect: default_auto_apply(),
            default_profile_id: None,
        }
    }
}

impl DaemonConfig {
    /// Load configuration from the config file, or create default if missing.
    pub async fn load(dirs: &ProjectDirs) -> Result<Self> {
        let config_path = dirs.config_dir().join("daemon.toml");

        if config_path.exists() {
            debug!(?config_path, "Loading configuration");
            let contents = fs::read_to_string(&config_path).await?;
            let config: DaemonConfig = toml::from_str(&contents)?;
            Ok(config)
        } else {
            info!(?config_path, "No config file found, using defaults");
            let config = DaemonConfig::default();
            // Optionally save defaults
            config.save(&config_path).await?;
            Ok(config)
        }
    }

    /// Save configuration to a file.
    pub async fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(path, contents).await?;
        debug!(?path, "Saved configuration");
        Ok(())
    }
}
