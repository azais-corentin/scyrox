//! Direct USB backend for mouse communication.

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, Mouse, MouseConfig, PollingRate};

use crate::backend::{Backend, DaemonInfo, ProfileInfo};

/// Backend that communicates directly with the mouse via USB.
pub struct DirectBackend {
    mouse: Mutex<Mouse>,
}

impl DirectBackend {
    /// Create a new direct backend.
    pub fn new() -> Result<Self> {
        let mouse = Mouse::open()?;
        Ok(Self {
            mouse: Mutex::new(mouse),
        })
    }
}

#[async_trait]
impl Backend for DirectBackend {
    async fn get_config(&self) -> Result<MouseConfig> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.get_config()?)
    }

    async fn get_battery(&self) -> Result<BatteryStatus> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.get_battery()?)
    }

    async fn get_firmware(&self) -> Result<FirmwareInfo> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.get_firmware_info()?)
    }

    async fn is_connected(&self) -> bool {
        true // If we got here, we're connected
    }

    async fn set_polling_rate(&self, rate: PollingRate) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_polling_rate(rate)?)
    }

    async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_lift_off_distance(lod)?)
    }

    async fn set_sleep_timeout(&self, seconds: u16) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_sleep_timeout(seconds)?)
    }

    async fn set_angle_snapping(&self, enabled: bool) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_angle_snapping(enabled)?)
    }

    async fn set_ripple_control(&self, enabled: bool) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_ripple_control(enabled)?)
    }

    async fn set_high_speed_mode(&self, enabled: bool) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_high_speed_mode(enabled)?)
    }

    async fn set_long_distance_mode(&self, enabled: bool) -> Result<()> {
        let mut mouse = self.mouse.lock().unwrap();
        Ok(mouse.set_long_distance_mode(enabled)?)
    }

    // Profile operations are not available in direct mode
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn get_profile(&self, _id: &str) -> Result<ProfileInfo> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn create_profile(&self, _name: &str, _set_default: bool) -> Result<ProfileInfo> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn apply_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn delete_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn set_default_profile(&self, _id: &str) -> Result<()> {
        Err(anyhow!(
            "Profile management requires the daemon. Start it with: scyroxctl daemon start"
        ))
    }

    async fn get_daemon_info(&self) -> Result<Option<DaemonInfo>> {
        Ok(None) // No daemon in direct mode
    }
}
