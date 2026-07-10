//! Shared backend abstraction and daemon client for Scyrox mouse configuration.
//!
//! This crate provides the [`Backend`] trait for abstracting direct USB vs daemon
//! communication, along with concrete implementations:
//!
//! - [`DaemonClient`]: connects to the scyroxd daemon via gRPC over Unix socket
//! - [`DirectBackend`]: communicates directly with the mouse via USB

mod daemon;
mod direct;

use anyhow::{Result, ensure};
use async_trait::async_trait;
use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, MouseConfig, PollingRate};
use serde::Serialize;

pub use daemon::{DaemonClient, EventStream};
pub use direct::DirectBackend;

/// Unified interface for mouse operations.
///
/// This trait is implemented by both the direct USB backend and the daemon client,
/// allowing consumers to work with either transparently.
#[async_trait]
pub trait Backend: Send + Sync {
    // Device info
    async fn get_config(&self) -> Result<MouseConfig>;
    async fn get_battery(&self) -> Result<BatteryStatus>;
    async fn get_firmware(&self) -> Result<FirmwareInfo>;
    async fn is_connected(&self) -> bool;

    // Configuration
    async fn set_polling_rate(&self, rate: PollingRate) -> Result<()>;
    async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()>;
    async fn set_sleep_timeout(&self, seconds: u16) -> Result<u16>;
    async fn set_angle_snapping(&self, enabled: bool) -> Result<()>;
    async fn set_ripple_control(&self, enabled: bool) -> Result<()>;
    async fn set_high_speed_mode(&self, enabled: bool) -> Result<()>;
    async fn set_long_distance_mode(&self, enabled: bool) -> Result<()>;

    // Profiles (only available with daemon)
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>>;
    async fn get_profile(&self, id: &str) -> Result<ProfileInfo>;
    async fn create_profile(&self, name: &str, set_default: bool) -> Result<ProfileInfo>;
    async fn apply_profile(&self, id: &str) -> Result<()>;
    async fn delete_profile(&self, id: &str) -> Result<()>;
    async fn set_default_profile(&self, id: &str) -> Result<()>;

    // Daemon info (only available with daemon)
    async fn get_daemon_info(&self) -> Result<Option<DaemonInfo>>;
}

/// Profile information.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub config: MouseConfig,
}

/// Daemon status information.
#[derive(Debug, Clone, Serialize)]
pub struct DaemonInfo {
    pub version: String,
    pub uptime_seconds: u64,
    pub connected: bool,
}

/// Effective daemon configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DaemonConfig {
    pub low_battery_threshold: u8,
}

impl TryFrom<scyrox_proto::DaemonConfig> for DaemonConfig {
    type Error = anyhow::Error;

    fn try_from(config: scyrox_proto::DaemonConfig) -> Result<Self> {
        ensure!(
            config.low_battery_threshold <= 100,
            "low_battery_threshold must be between 0 and 100"
        );
        Ok(Self {
            low_battery_threshold: config.low_battery_threshold as u8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_config_accepts_percentage_boundaries() {
        for low_battery_threshold in [0, 100] {
            let config = DaemonConfig::try_from(scyrox_proto::DaemonConfig {
                low_battery_threshold,
            })
            .unwrap();

            assert_eq!(
                config,
                DaemonConfig {
                    low_battery_threshold: low_battery_threshold as u8,
                }
            );
        }
    }

    #[test]
    fn daemon_config_rejects_percentage_above_one_hundred() {
        let error = DaemonConfig::try_from(scyrox_proto::DaemonConfig {
            low_battery_threshold: 101,
        })
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "low_battery_threshold must be between 0 and 100"
        );
    }
}
