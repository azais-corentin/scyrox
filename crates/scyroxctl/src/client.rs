//! gRPC client for connecting to the scyroxd daemon.

use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;
use directories::ProjectDirs;
use hyper_util::rt::TokioIo;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::GenericFilePath;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, MouseConfig, PollingRate};
use scyrox_proto::scyrox_client::ScyroxClient;
use scyrox_proto::*;

use crate::backend::{Backend, DaemonInfo, ProfileInfo};

/// Default socket name.
const SOCKET_NAME: &str = "scyroxd.sock";

/// Client that connects to the scyroxd daemon via gRPC over IPC.
pub struct DaemonClient {
    client: Mutex<ScyroxClient<Channel>>,
}

impl DaemonClient {
    /// Connect to the daemon.
    pub async fn connect() -> Result<Self> {
        let socket_path = get_socket_path()?;

        // Create a channel that connects over Unix socket
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_: Uri| {
                let path = socket_path.clone();
                async move {
                    let name = path.as_os_str().to_fs_name::<GenericFilePath>()?;
                    let stream = LocalSocketStream::connect(name).await?;
                    Ok::<_, std::io::Error>(TokioIo::new(stream))
                }
            }))
            .await
            .context("Failed to connect to daemon")?;

        let client = ScyroxClient::new(channel);

        Ok(Self {
            client: Mutex::new(client),
        })
    }

    /// Send shutdown command to daemon.
    pub async fn shutdown(&self) -> Result<()> {
        let mut client = self.client.lock().await;
        let _ = client.shutdown(Empty {}).await;
        Ok(())
    }

    /// Get daemon info directly (for daemon status command).
    pub async fn get_info(&self) -> Result<scyrox_proto::DaemonInfo> {
        let mut client = self.client.lock().await;
        Ok(client.get_daemon_info(Empty {}).await?.into_inner())
    }
}

#[async_trait]
impl Backend for DaemonClient {
    async fn get_config(&self) -> Result<MouseConfig> {
        let mut client = self.client.lock().await;
        let response = client.get_config(Empty {}).await?.into_inner();
        proto_to_config(&response)
    }

    async fn get_battery(&self) -> Result<BatteryStatus> {
        let mut client = self.client.lock().await;
        let response = client.get_battery(Empty {}).await?.into_inner();
        Ok(BatteryStatus {
            voltage_mv: response.voltage_mv as u16,
            percentage: response.percentage as u8,
        })
    }

    async fn get_firmware(&self) -> Result<FirmwareInfo> {
        let mut client = self.client.lock().await;
        let response = client.get_firmware(Empty {}).await?.into_inner();
        Ok(FirmwareInfo {
            mouse_version: response.mouse_version,
            receiver_version: response.receiver_version,
        })
    }

    async fn is_connected(&self) -> bool {
        let mut client = self.client.lock().await;
        client
            .get_status(Empty {})
            .await
            .map(|r| r.into_inner().connected)
            .unwrap_or(false)
    }

    async fn set_polling_rate(&self, rate: PollingRate) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_polling_rate(SetPollingRateRequest {
                rate: polling_rate_to_proto(rate) as i32,
            })
            .await?;
        Ok(())
    }

    async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_lift_off_distance(SetLiftOffDistanceRequest {
                distance: lod_to_proto(lod) as i32,
            })
            .await?;
        Ok(())
    }

    async fn set_sleep_timeout(&self, seconds: u16) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_sleep_timeout(SetSleepTimeoutRequest {
                seconds: seconds as u32,
            })
            .await?;
        Ok(())
    }

    async fn set_angle_snapping(&self, enabled: bool) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_angle_snapping(SetBoolRequest { enabled })
            .await?;
        Ok(())
    }

    async fn set_ripple_control(&self, enabled: bool) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_ripple_control(SetBoolRequest { enabled })
            .await?;
        Ok(())
    }

    async fn set_high_speed_mode(&self, enabled: bool) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_high_speed_mode(SetBoolRequest { enabled })
            .await?;
        Ok(())
    }

    async fn set_long_distance_mode(&self, enabled: bool) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_long_distance_mode(SetBoolRequest { enabled })
            .await?;
        Ok(())
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        let mut client = self.client.lock().await;
        let response = client.list_profiles(Empty {}).await?.into_inner();
        response
            .profiles
            .into_iter()
            .map(proto_to_profile_info)
            .collect()
    }

    async fn get_profile(&self, id: &str) -> Result<ProfileInfo> {
        let mut client = self.client.lock().await;
        let response = client
            .get_profile(GetProfileRequest { id: id.to_string() })
            .await?
            .into_inner();
        proto_to_profile_info(response)
    }

    async fn create_profile(&self, name: &str, set_default: bool) -> Result<ProfileInfo> {
        // First get current config
        let config = self.get_config().await?;

        let mut client = self.client.lock().await;
        let response = client
            .create_profile(CreateProfileRequest {
                name: name.to_string(),
                config: Some(config_to_proto(&config)),
                set_as_default: set_default,
            })
            .await?
            .into_inner();
        proto_to_profile_info(response)
    }

    async fn apply_profile(&self, id: &str) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .apply_profile(ApplyProfileRequest { id: id.to_string() })
            .await?;
        Ok(())
    }

    async fn delete_profile(&self, id: &str) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .delete_profile(DeleteProfileRequest { id: id.to_string() })
            .await?;
        Ok(())
    }

    async fn set_default_profile(&self, id: &str) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_default_profile(SetDefaultProfileRequest { id: id.to_string() })
            .await?;
        Ok(())
    }

    async fn get_daemon_info(&self) -> Result<Option<DaemonInfo>> {
        let mut client = self.client.lock().await;
        let response = client.get_daemon_info(Empty {}).await?.into_inner();
        Ok(Some(DaemonInfo {
            version: response.version,
            uptime_seconds: response.uptime_seconds,
            connected: response
                .device_status
                .map(|s| s.connected)
                .unwrap_or(false),
        }))
    }
}

/// Get the socket path.
fn get_socket_path() -> Result<PathBuf> {
    // Try XDG_RUNTIME_DIR first
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime_dir).join("scyrox").join(SOCKET_NAME));
    }

    // Fall back to state directory
    let dirs = ProjectDirs::from("", "", "scyrox")
        .context("Failed to determine project directories")?;
    let state_dir = dirs.state_dir().unwrap_or_else(|| dirs.data_local_dir());
    Ok(state_dir.join(SOCKET_NAME))
}

// =============================================================================
// Conversion helpers
// =============================================================================

fn proto_to_config(proto: &scyrox_proto::MouseConfig) -> Result<MouseConfig> {
    Ok(MouseConfig {
        polling_rate: proto_to_polling_rate(proto.polling_rate)?,
        lift_off_distance: proto_to_lod(proto.lift_off_distance)?,
        sleep_timeout_seconds: proto.sleep_timeout_seconds as u16,
        angle_snapping: proto.angle_snapping,
        ripple_control: proto.ripple_control,
        high_speed_mode: proto.high_speed_mode,
        long_distance_mode: proto.long_distance_mode,
    })
}

fn config_to_proto(config: &MouseConfig) -> scyrox_proto::MouseConfig {
    scyrox_proto::MouseConfig {
        polling_rate: polling_rate_to_proto(config.polling_rate) as i32,
        lift_off_distance: lod_to_proto(config.lift_off_distance) as i32,
        sleep_timeout_seconds: config.sleep_timeout_seconds as u32,
        angle_snapping: config.angle_snapping,
        ripple_control: config.ripple_control,
        high_speed_mode: config.high_speed_mode,
        long_distance_mode: config.long_distance_mode,
    }
}

fn proto_to_polling_rate(value: i32) -> Result<PollingRate> {
    match scyrox_proto::PollingRate::try_from(value) {
        Ok(scyrox_proto::PollingRate::PollingRate125) => Ok(PollingRate::Hz125),
        Ok(scyrox_proto::PollingRate::PollingRate250) => Ok(PollingRate::Hz250),
        Ok(scyrox_proto::PollingRate::PollingRate500) => Ok(PollingRate::Hz500),
        Ok(scyrox_proto::PollingRate::PollingRate1000) => Ok(PollingRate::Hz1000),
        Ok(scyrox_proto::PollingRate::PollingRate2000) => Ok(PollingRate::Hz2000),
        Ok(scyrox_proto::PollingRate::PollingRate4000) => Ok(PollingRate::Hz4000),
        Ok(scyrox_proto::PollingRate::PollingRate8000) => Ok(PollingRate::Hz8000),
        _ => anyhow::bail!("Invalid polling rate: {}", value),
    }
}

fn polling_rate_to_proto(rate: PollingRate) -> scyrox_proto::PollingRate {
    match rate {
        PollingRate::Hz125 => scyrox_proto::PollingRate::PollingRate125,
        PollingRate::Hz250 => scyrox_proto::PollingRate::PollingRate250,
        PollingRate::Hz500 => scyrox_proto::PollingRate::PollingRate500,
        PollingRate::Hz1000 => scyrox_proto::PollingRate::PollingRate1000,
        PollingRate::Hz2000 => scyrox_proto::PollingRate::PollingRate2000,
        PollingRate::Hz4000 => scyrox_proto::PollingRate::PollingRate4000,
        PollingRate::Hz8000 => scyrox_proto::PollingRate::PollingRate8000,
    }
}

fn proto_to_lod(value: i32) -> Result<LiftOffDistance> {
    match scyrox_proto::LiftOffDistance::try_from(value) {
        Ok(scyrox_proto::LiftOffDistance::Low) => Ok(LiftOffDistance::Low),
        Ok(scyrox_proto::LiftOffDistance::Medium) => Ok(LiftOffDistance::Medium),
        Ok(scyrox_proto::LiftOffDistance::High) => Ok(LiftOffDistance::High),
        _ => anyhow::bail!("Invalid lift-off distance: {}", value),
    }
}

fn lod_to_proto(lod: LiftOffDistance) -> scyrox_proto::LiftOffDistance {
    match lod {
        LiftOffDistance::Low => scyrox_proto::LiftOffDistance::Low,
        LiftOffDistance::Medium => scyrox_proto::LiftOffDistance::Medium,
        LiftOffDistance::High => scyrox_proto::LiftOffDistance::High,
    }
}

fn proto_to_profile_info(proto: scyrox_proto::Profile) -> Result<ProfileInfo> {
    let config = proto
        .config
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Profile missing config"))?;

    Ok(ProfileInfo {
        id: proto.id,
        name: proto.name,
        is_default: proto.is_default,
        config: proto_to_config(config)?,
    })
}
