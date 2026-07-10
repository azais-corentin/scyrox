//! gRPC client for connecting to the scyroxd daemon.

use anyhow::{Context, Result, anyhow, ensure};
use async_trait::async_trait;
use hyper_util::rt::TokioIo;
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::tokio::prelude::*;
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

use scyrox::paths::get_socket_path;
use scyrox::{BatteryStatus, FirmwareInfo, LiftOffDistance, MouseConfig, PollingRate};
use scyrox_proto::scyrox_client::ScyroxClient;
use scyrox_proto::{
    ApplyProfileRequest, DeleteProfileRequest, Empty, GetProfileRequest,
    LiftOffDistance as ProtoLod, PollingRate as ProtoRate, SetBoolRequest,
    SetDefaultProfileRequest, SetLiftOffDistanceRequest, SetLowBatteryThresholdRequest,
    SetPollingRateRequest, SetSleepTimeoutRequest,
};

use crate::{Backend, DaemonConfig, DaemonInfo, ProfileInfo};

/// Client that connects to the scyroxd daemon via gRPC over IPC.
pub struct DaemonClient {
    client: Mutex<ScyroxClient<Channel>>,
}

/// Event stream returned by [`DaemonClient::watch_events`].
pub type EventStream = tonic::codec::Streaming<scyrox_proto::Event>;

impl DaemonClient {
    /// Connect to the daemon.
    pub async fn connect() -> Result<Self> {
        let socket_path = get_socket_path().ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to determine socket path: no runtime or state directory available"
            )
        })?;

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

    /// Get the daemon's effective configuration.
    pub async fn get_daemon_config(&self) -> Result<DaemonConfig> {
        let mut client = self.client.lock().await;
        let response = client.get_daemon_config(Empty {}).await?.into_inner();
        DaemonConfig::try_from(response).context("daemon returned invalid configuration")
    }

    /// Set and persist the daemon's low-battery threshold.
    pub async fn set_low_battery_threshold(&self, percentage: u8) -> Result<()> {
        ensure!(
            percentage <= 100,
            "low_battery_threshold must be between 0 and 100"
        );
        let mut client = self.client.lock().await;
        client
            .set_low_battery_threshold(SetLowBatteryThresholdRequest {
                percentage: percentage as u32,
            })
            .await?;
        Ok(())
    }

    /// Subscribe to the daemon event stream.
    pub async fn watch_events(&self) -> Result<EventStream> {
        let mut client = self.client.lock().await;
        let response = client.watch_events(scyrox_proto::Empty {}).await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl Backend for DaemonClient {
    async fn get_config(&self) -> Result<MouseConfig> {
        let mut client = self.client.lock().await;
        let response = client.get_config(Empty {}).await?.into_inner();
        MouseConfig::try_from(&response).map_err(|e| anyhow!("Failed to convert config: {}", e))
    }

    async fn get_battery(&self) -> Result<BatteryStatus> {
        let mut client = self.client.lock().await;
        let response = client.get_battery(Empty {}).await?.into_inner();
        Ok(BatteryStatus {
            voltage_mv: response.voltage_mv as u16,
            percentage: response.percentage as u8,
            charging: response.charging,
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
                rate: ProtoRate::from(rate) as i32,
            })
            .await?;
        Ok(())
    }

    async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()> {
        let mut client = self.client.lock().await;
        client
            .set_lift_off_distance(SetLiftOffDistanceRequest {
                distance: ProtoLod::from(lod) as i32,
            })
            .await?;
        Ok(())
    }

    async fn set_sleep_timeout(&self, seconds: u16) -> Result<u16> {
        let mut client = self.client.lock().await;
        let response = client
            .set_sleep_timeout(SetSleepTimeoutRequest {
                seconds: seconds as u32,
            })
            .await?;
        Ok(response.into_inner().actual_seconds as u16)
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
            .create_profile(scyrox_proto::CreateProfileRequest {
                name: name.to_string(),
                config: Some(scyrox_proto::MouseConfig::from(&config)),
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
            connected: response.device_status.map(|s| s.connected).unwrap_or(false),
        }))
    }
}

// =============================================================================
// Conversion helpers
// =============================================================================

fn proto_to_profile_info(proto: scyrox_proto::Profile) -> Result<ProfileInfo> {
    let config = proto
        .config
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Profile missing config"))?;

    Ok(ProfileInfo {
        id: proto.id,
        name: proto.name,
        is_default: proto.is_default,
        config: MouseConfig::try_from(config)
            .map_err(|e| anyhow!("Failed to convert config: {}", e))?,
    })
}
