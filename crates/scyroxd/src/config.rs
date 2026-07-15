//! Daemon configuration management.

use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info};

/// Daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Daemon-owned battery percentage threshold for low-battery alerts (default: 10).
    #[serde(default = "default_low_battery_threshold")]
    pub low_battery_threshold: u8,

    /// Optional JSON Lines destination for daemon battery observations.
    #[serde(default)]
    pub battery_log_path: Option<PathBuf>,

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
    10
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
            battery_log_path: None,
            battery_poll_interval_secs: default_battery_poll_interval(),
            auto_apply_on_connect: default_auto_apply(),
            default_profile_id: None,
        }
    }
}

impl DaemonConfig {
    /// Canonical config file path (shared by startup load and RPC persistence).
    pub fn path(dirs: &ProjectDirs) -> PathBuf {
        dirs.config_dir().join("daemon.toml")
    }

    /// Load configuration from the config file, or create default if missing.
    pub async fn load(dirs: &ProjectDirs) -> Result<Self> {
        let config_path = Self::path(dirs);

        if config_path.exists() {
            debug!(?config_path, "Loading configuration");
            let contents = fs::read_to_string(&config_path).await?;
            let config: DaemonConfig = toml::from_str(&contents)?;
            config.validate()?;
            Ok(config)
        } else {
            info!(?config_path, "No config file found, using defaults");
            let config = DaemonConfig::default();
            // Optionally save defaults
            config.save(&config_path).await?;
            Ok(config)
        }
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.low_battery_threshold <= 100,
            "low_battery_threshold must be between 0 and 100"
        );
        if let Some(path) = &self.battery_log_path {
            ensure!(
                !path.as_os_str().is_empty(),
                "battery_log_path must not be empty"
            );
            ensure!(
                path.to_str().is_some(),
                "battery_log_path must be valid UTF-8"
            );
        }
        Ok(())
    }

    /// Resolve a configured battery log path beneath the daemon config directory.
    pub fn resolved_battery_log_path(&self, config_dir: &Path) -> Option<PathBuf> {
        self.battery_log_path.as_ref().map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                config_dir.join(path)
            }
        })
    }

    /// Save configuration to a file.
    pub async fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::fs_util::write_atomic(path, &toml::to_string_pretty(self)?).await?;
        debug!(?path, "Saved configuration");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_low_battery_threshold_is_ten_percent() {
        assert_eq!(DaemonConfig::default().low_battery_threshold, 10);
    }

    #[test]
    fn missing_low_battery_threshold_defaults_to_ten_percent() {
        let config: DaemonConfig = toml::from_str(
            "battery_poll_interval_secs = 30\n\
             auto_apply_on_connect = false\n",
        )
        .unwrap();

        assert_eq!(config.low_battery_threshold, 10);
    }

    #[test]
    fn threshold_above_one_hundred_is_rejected() {
        let config = DaemonConfig {
            low_battery_threshold: 101,
            ..DaemonConfig::default()
        };

        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "low_battery_threshold must be between 0 and 100"
        );
    }

    #[test]
    fn missing_battery_log_path_defaults_to_none() {
        let config: DaemonConfig = toml::from_str("battery_poll_interval_secs = 30").unwrap();

        assert_eq!(config.battery_log_path, None);
    }

    #[test]
    fn empty_battery_log_path_is_rejected() {
        let config = DaemonConfig {
            battery_log_path: Some(PathBuf::new()),
            ..DaemonConfig::default()
        };

        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "battery_log_path must not be empty"
        );
    }

    #[test]
    fn battery_log_path_roundtrips_through_toml() {
        let config = DaemonConfig {
            battery_log_path: Some(PathBuf::from("captures/battery.jsonl")),
            ..DaemonConfig::default()
        };

        let serialized = toml::to_string(&config).unwrap();
        let decoded: DaemonConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(decoded.battery_log_path, config.battery_log_path);
    }

    #[test]
    fn relative_battery_log_path_resolves_beneath_config_directory() {
        let config = DaemonConfig {
            battery_log_path: Some(PathBuf::from("captures/battery.jsonl")),
            ..DaemonConfig::default()
        };

        assert_eq!(
            config.resolved_battery_log_path(Path::new("/config")),
            Some(PathBuf::from("/config/captures/battery.jsonl"))
        );
    }

    #[test]
    fn absolute_battery_log_path_remains_unchanged() {
        let config = DaemonConfig {
            battery_log_path: Some(PathBuf::from("/captures/battery.jsonl")),
            ..DaemonConfig::default()
        };

        assert_eq!(
            config.resolved_battery_log_path(Path::new("/config")),
            config.battery_log_path
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_battery_log_path_is_rejected() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let config = DaemonConfig {
            battery_log_path: Some(PathBuf::from(OsString::from_vec(vec![0xff]))),
            ..DaemonConfig::default()
        };

        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "battery_log_path must be valid UTF-8"
        );
    }

    #[tokio::test]
    async fn save_roundtrips_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("scyroxd-config-test-{}", std::process::id()));
        let path = dir.join("daemon.toml");

        let config = DaemonConfig {
            low_battery_threshold: 42,
            ..DaemonConfig::default()
        };
        config.save(&path).await.unwrap();

        let loaded: DaemonConfig =
            toml::from_str(&fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(loaded.low_battery_threshold, 42);
        assert!(!path.with_extension("toml.tmp").exists());

        fs::remove_dir_all(&dir).await.unwrap();
    }
}
