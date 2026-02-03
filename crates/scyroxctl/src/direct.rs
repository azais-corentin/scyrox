//! Direct USB backend for mouse communication.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, Mouse, MouseConfig, PollingRate};
use tokio::sync::Mutex;

use crate::backend::{Backend, DaemonInfo, ProfileInfo};

const PROFILE_REQUIRES_DAEMON: &str =
    "Profile management requires the daemon. Start it with: scyroxctl daemon start";

/// Backend that communicates directly with the mouse via USB.
pub struct DirectBackend {
    mouse: Mutex<Mouse>,
}

impl DirectBackend {
    /// Create a new direct backend.
    pub async fn new() -> Result<Self> {
        let mouse = Mouse::open().await?;
        Ok(Self {
            mouse: Mutex::new(mouse),
        })
    }
}

#[async_trait]
impl Backend for DirectBackend {
    async fn get_config(&self) -> Result<MouseConfig> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.get_config().await?)
    }

    async fn get_battery(&self) -> Result<BatteryStatus> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.get_battery().await?)
    }

    async fn get_firmware(&self) -> Result<FirmwareInfo> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.get_firmware_info().await?)
    }

    async fn is_connected(&self) -> bool {
        true // If we got here, we're connected
    }

    async fn set_polling_rate(&self, rate: PollingRate) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_polling_rate(rate).await?)
    }

    async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_lift_off_distance(lod).await?)
    }

    async fn set_sleep_timeout(&self, seconds: u16) -> Result<u16> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_sleep_timeout(seconds).await?)
    }

    async fn set_angle_snapping(&self, enabled: bool) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_angle_snapping(enabled).await?)
    }

    async fn set_ripple_control(&self, enabled: bool) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_ripple_control(enabled).await?)
    }

    async fn set_high_speed_mode(&self, enabled: bool) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_high_speed_mode(enabled).await?)
    }

    async fn set_long_distance_mode(&self, enabled: bool) -> Result<()> {
        let mouse = self.mouse.lock().await;
        Ok(mouse.set_long_distance_mode(enabled).await?)
    }

    // Profile operations are not available in direct mode
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn get_profile(&self, _id: &str) -> Result<ProfileInfo> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn create_profile(&self, _name: &str, _set_default: bool) -> Result<ProfileInfo> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn apply_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn delete_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn set_default_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(PROFILE_REQUIRES_DAEMON))
    }

    async fn get_daemon_info(&self) -> Result<Option<DaemonInfo>> {
        Ok(None) // No daemon in direct mode
    }
}
