//! Backend trait for abstracting direct vs daemon access.

use anyhow::Result;
use async_trait::async_trait;
use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, MouseConfig, PollingRate};
use serde::Serialize;

/// Unified interface for mouse operations.
///
/// This trait is implemented by both the direct USB backend and the daemon client,
/// allowing the CLI commands to work with either.
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
