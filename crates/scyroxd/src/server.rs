//! gRPC service implementation.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use directories::ProjectDirs;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, instrument, warn};

use scyrox::Mouse;
use scyrox_proto::*;

use crate::config::DaemonConfig;
use crate::profiles::{ProfileConfig, ProfileStore};

/// The gRPC service implementation.
pub struct ScyroxService {
    /// Mouse device handle (protected by mutex for thread safety).
    mouse: Arc<Mutex<Option<Mouse>>>,
    /// Profile storage.
    profiles: ProfileStore,
    /// Daemon configuration.
    config: DaemonConfig,
    /// Daemon start time.
    start_time: Instant,
}

impl ScyroxService {
    /// Create a new service instance.
    pub fn new(config: DaemonConfig, dirs: ProjectDirs) -> Result<Self> {
        // Try to open the mouse, but don't fail if not connected
        let mouse = match Mouse::open() {
            Ok(m) => {
                info!("Mouse connected");
                Some(m)
            }
            Err(e) => {
                warn!("Mouse not connected: {}", e);
                None
            }
        };

        Ok(Self {
            mouse: Arc::new(Mutex::new(mouse)),
            profiles: ProfileStore::new(&dirs),
            config,
            start_time: Instant::now(),
        })
    }

    /// Ensure mouse is connected, attempting to reconnect if needed.
    async fn ensure_mouse(&self) -> Result<(), Status> {
        let mut guard = self.mouse.lock().await;
        if guard.is_none() {
            match Mouse::open() {
                Ok(m) => {
                    info!("Mouse reconnected");
                    *guard = Some(m);
                }
                Err(e) => {
                    return Err(Status::unavailable(format!("Mouse not connected: {}", e)));
                }
            }
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl Scyrox for ScyroxService {
    // =========================================================================
    // Device Status
    // =========================================================================

    #[instrument(skip(self, _request))]
    async fn get_status(&self, _request: Request<Empty>) -> Result<Response<DeviceStatus>, Status> {
        let guard = self.mouse.lock().await;
        let (connected, mode) = match guard.as_ref() {
            Some(m) => {
                let mode = match m.connection_mode() {
                    scyrox::ConnectionMode::Wired => ConnectionMode::Wired,
                    scyrox::ConnectionMode::Wireless => ConnectionMode::Wireless,
                };
                (true, mode as i32)
            }
            None => (false, ConnectionMode::Unspecified as i32),
        };

        Ok(Response::new(DeviceStatus {
            connected,
            connection_mode: mode,
        }))
    }

    #[instrument(skip(self, _request))]
    async fn get_battery(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<BatteryStatus>, Status> {
        self.ensure_mouse().await?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        let battery = mouse
            .get_battery()
            .map_err(|e| Status::internal(format!("Failed to read battery: {}", e)))?;

        Ok(Response::new(BatteryStatus {
            voltage_mv: battery.voltage_mv as u32,
            percentage: battery.percentage as u32,
        }))
    }

    #[instrument(skip(self, _request))]
    async fn get_firmware(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<FirmwareInfo>, Status> {
        self.ensure_mouse().await?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        let firmware = mouse
            .get_firmware_info()
            .map_err(|e| Status::internal(format!("Failed to read firmware: {}", e)))?;

        Ok(Response::new(FirmwareInfo {
            mouse_version: firmware.mouse_version,
            receiver_version: firmware.receiver_version,
        }))
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    #[instrument(skip(self, _request))]
    async fn get_config(&self, _request: Request<Empty>) -> Result<Response<MouseConfig>, Status> {
        self.ensure_mouse().await?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        let config = mouse
            .get_config()
            .map_err(|e| Status::internal(format!("Failed to read config: {}", e)))?;

        Ok(Response::new(convert_config_to_proto(&config)))
    }

    #[instrument(skip(self, request))]
    async fn set_config(&self, request: Request<MouseConfig>) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let proto_config = request.into_inner();
        let config = convert_proto_to_config(&proto_config)?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_config(&config)
            .map_err(|e| Status::internal(format!("Failed to set config: {}", e)))?;

        info!("Configuration updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_polling_rate(
        &self,
        request: Request<SetPollingRateRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let rate = convert_polling_rate(request.into_inner().rate())?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_polling_rate(rate)
            .map_err(|e| Status::internal(format!("Failed to set polling rate: {}", e)))?;

        info!(?rate, "Polling rate updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_lift_off_distance(
        &self,
        request: Request<SetLiftOffDistanceRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let lod = convert_lift_off_distance(request.into_inner().distance())?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_lift_off_distance(lod)
            .map_err(|e| Status::internal(format!("Failed to set lift-off distance: {}", e)))?;

        info!(?lod, "Lift-off distance updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_sleep_timeout(
        &self,
        request: Request<SetSleepTimeoutRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let seconds = request.into_inner().seconds as u16;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_sleep_timeout(seconds)
            .map_err(|e| Status::internal(format!("Failed to set sleep timeout: {}", e)))?;

        info!(seconds, "Sleep timeout updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_angle_snapping(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let enabled = request.into_inner().enabled;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_angle_snapping(enabled)
            .map_err(|e| Status::internal(format!("Failed to set angle snapping: {}", e)))?;

        info!(enabled, "Angle snapping updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_ripple_control(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let enabled = request.into_inner().enabled;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_ripple_control(enabled)
            .map_err(|e| Status::internal(format!("Failed to set ripple control: {}", e)))?;

        info!(enabled, "Ripple control updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_high_speed_mode(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let enabled = request.into_inner().enabled;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_high_speed_mode(enabled)
            .map_err(|e| Status::internal(format!("Failed to set high speed mode: {}", e)))?;

        info!(enabled, "High speed mode updated");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_long_distance_mode(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let enabled = request.into_inner().enabled;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_long_distance_mode(enabled)
            .map_err(|e| Status::internal(format!("Failed to set long distance mode: {}", e)))?;

        info!(enabled, "Long distance mode updated");
        Ok(Response::new(Empty {}))
    }

    // =========================================================================
    // Profiles
    // =========================================================================

    #[instrument(skip(self, _request))]
    async fn list_profiles(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ProfileList>, Status> {
        let profiles = self
            .profiles
            .list()
            .await
            .map_err(|e| Status::internal(format!("Failed to list profiles: {}", e)))?;

        let proto_profiles = profiles.into_iter().map(convert_profile_to_proto).collect();

        Ok(Response::new(ProfileList {
            profiles: proto_profiles,
        }))
    }

    #[instrument(skip(self, request))]
    async fn get_profile(
        &self,
        request: Request<GetProfileRequest>,
    ) -> Result<Response<Profile>, Status> {
        let id = request.into_inner().id;

        let profile = self
            .profiles
            .get(&id)
            .await
            .map_err(|e| Status::not_found(format!("Profile not found: {}", e)))?;

        Ok(Response::new(convert_profile_to_proto(profile)))
    }

    #[instrument(skip(self, request))]
    async fn create_profile(
        &self,
        request: Request<CreateProfileRequest>,
    ) -> Result<Response<Profile>, Status> {
        let req = request.into_inner();

        let config = req
            .config
            .ok_or_else(|| Status::invalid_argument("Config is required"))?;

        let profile_config = convert_proto_to_profile_config(&config)?;

        let mut profile = self
            .profiles
            .create(req.name, profile_config)
            .await
            .map_err(|e| Status::internal(format!("Failed to create profile: {}", e)))?;

        if req.set_as_default {
            self.profiles
                .set_default(&profile.id)
                .await
                .map_err(|e| Status::internal(format!("Failed to set default: {}", e)))?;
            profile.is_default = true;
        }

        Ok(Response::new(convert_profile_to_proto(profile)))
    }

    #[instrument(skip(self, request))]
    async fn update_profile(
        &self,
        request: Request<UpdateProfileRequest>,
    ) -> Result<Response<Profile>, Status> {
        let req = request.into_inner();

        let config = req
            .config
            .map(|c| convert_proto_to_profile_config(&c))
            .transpose()?;

        let profile = self
            .profiles
            .update(&req.id, req.name, config)
            .await
            .map_err(|e| Status::internal(format!("Failed to update profile: {}", e)))?;

        Ok(Response::new(convert_profile_to_proto(profile)))
    }

    #[instrument(skip(self, request))]
    async fn delete_profile(
        &self,
        request: Request<DeleteProfileRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = request.into_inner().id;

        self.profiles
            .delete(&id)
            .await
            .map_err(|e| Status::internal(format!("Failed to delete profile: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn apply_profile(
        &self,
        request: Request<ApplyProfileRequest>,
    ) -> Result<Response<Empty>, Status> {
        self.ensure_mouse().await?;

        let id = request.into_inner().id;

        let profile = self
            .profiles
            .get(&id)
            .await
            .map_err(|e| Status::not_found(format!("Profile not found: {}", e)))?;

        let config = profile_config_to_mouse_config(&profile.config)?;

        let mut guard = self.mouse.lock().await;
        let mouse = guard.as_mut().unwrap();

        mouse
            .set_config(&config)
            .map_err(|e| Status::internal(format!("Failed to apply profile: {}", e)))?;

        info!(id, "Profile applied");
        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_default_profile(
        &self,
        request: Request<SetDefaultProfileRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = request.into_inner().id;

        self.profiles
            .set_default(&id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set default profile: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    // =========================================================================
    // Event Streaming
    // =========================================================================

    type WatchEventsStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<Event, Status>> + Send>>;

    #[instrument(skip(self, _request))]
    async fn watch_events(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::WatchEventsStream>, Status> {
        let (_tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Status>>(4);
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    // =========================================================================
    // Daemon Management
    // =========================================================================

    #[instrument(skip(self, _request))]
    async fn get_daemon_info(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<DaemonInfo>, Status> {
        let guard = self.mouse.lock().await;
        let (connected, mode) = match guard.as_ref() {
            Some(m) => {
                let mode = match m.connection_mode() {
                    scyrox::ConnectionMode::Wired => ConnectionMode::Wired,
                    scyrox::ConnectionMode::Wireless => ConnectionMode::Wireless,
                };
                (true, mode as i32)
            }
            None => (false, ConnectionMode::Unspecified as i32),
        };

        Ok(Response::new(DaemonInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            device_status: Some(DeviceStatus {
                connected,
                connection_mode: mode,
            }),
            active_profile_id: None, // TODO: Track active profile
        }))
    }

    #[instrument(skip(self, _request))]
    async fn shutdown(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        info!("Shutdown requested");
        // TODO: Graceful shutdown
        std::process::exit(0);
    }
}

// =============================================================================
// Conversion Helpers
// =============================================================================

fn convert_config_to_proto(config: &scyrox::MouseConfig) -> MouseConfig {
    MouseConfig {
        polling_rate: polling_rate_to_proto(config.polling_rate) as i32,
        lift_off_distance: lift_off_distance_to_proto(config.lift_off_distance) as i32,
        sleep_timeout_seconds: config.sleep_timeout_seconds as u32,
        angle_snapping: config.angle_snapping,
        ripple_control: config.ripple_control,
        high_speed_mode: config.high_speed_mode,
        long_distance_mode: config.long_distance_mode,
    }
}

fn convert_proto_to_config(proto: &MouseConfig) -> Result<scyrox::MouseConfig, Status> {
    Ok(scyrox::MouseConfig {
        polling_rate: convert_polling_rate(PollingRate::try_from(proto.polling_rate).unwrap_or(PollingRate::Unspecified))?,
        lift_off_distance: convert_lift_off_distance(LiftOffDistance::try_from(proto.lift_off_distance).unwrap_or(LiftOffDistance::Unspecified))?,
        sleep_timeout_seconds: proto.sleep_timeout_seconds as u16,
        angle_snapping: proto.angle_snapping,
        ripple_control: proto.ripple_control,
        high_speed_mode: proto.high_speed_mode,
        long_distance_mode: proto.long_distance_mode,
    })
}

fn convert_polling_rate(rate: PollingRate) -> Result<scyrox::PollingRate, Status> {
    match rate {
        PollingRate::Unspecified => Err(Status::invalid_argument("Polling rate not specified")),
        PollingRate::PollingRate125 => Ok(scyrox::PollingRate::Hz125),
        PollingRate::PollingRate250 => Ok(scyrox::PollingRate::Hz250),
        PollingRate::PollingRate500 => Ok(scyrox::PollingRate::Hz500),
        PollingRate::PollingRate1000 => Ok(scyrox::PollingRate::Hz1000),
        PollingRate::PollingRate2000 => Ok(scyrox::PollingRate::Hz2000),
        PollingRate::PollingRate4000 => Ok(scyrox::PollingRate::Hz4000),
        PollingRate::PollingRate8000 => Ok(scyrox::PollingRate::Hz8000),
    }
}

fn polling_rate_to_proto(rate: scyrox::PollingRate) -> PollingRate {
    match rate {
        scyrox::PollingRate::Hz125 => PollingRate::PollingRate125,
        scyrox::PollingRate::Hz250 => PollingRate::PollingRate250,
        scyrox::PollingRate::Hz500 => PollingRate::PollingRate500,
        scyrox::PollingRate::Hz1000 => PollingRate::PollingRate1000,
        scyrox::PollingRate::Hz2000 => PollingRate::PollingRate2000,
        scyrox::PollingRate::Hz4000 => PollingRate::PollingRate4000,
        scyrox::PollingRate::Hz8000 => PollingRate::PollingRate8000,
    }
}

fn convert_lift_off_distance(lod: LiftOffDistance) -> Result<scyrox::LiftOffDistance, Status> {
    match lod {
        LiftOffDistance::Unspecified => {
            Err(Status::invalid_argument("Lift-off distance not specified"))
        }
        LiftOffDistance::Low => Ok(scyrox::LiftOffDistance::Low),
        LiftOffDistance::Medium => Ok(scyrox::LiftOffDistance::Medium),
        LiftOffDistance::High => Ok(scyrox::LiftOffDistance::High),
    }
}

fn lift_off_distance_to_proto(lod: scyrox::LiftOffDistance) -> LiftOffDistance {
    match lod {
        scyrox::LiftOffDistance::Low => LiftOffDistance::Low,
        scyrox::LiftOffDistance::Medium => LiftOffDistance::Medium,
        scyrox::LiftOffDistance::High => LiftOffDistance::High,
    }
}

fn convert_profile_to_proto(profile: crate::profiles::Profile) -> Profile {
    Profile {
        id: profile.id,
        name: profile.name,
        config: Some(MouseConfig {
            polling_rate: hz_to_polling_rate(profile.config.polling_rate_hz) as i32,
            lift_off_distance: mm_to_lift_off_distance(profile.config.lift_off_distance_mm) as i32,
            sleep_timeout_seconds: profile.config.sleep_timeout_seconds as u32,
            angle_snapping: profile.config.angle_snapping,
            ripple_control: profile.config.ripple_control,
            high_speed_mode: profile.config.high_speed_mode,
            long_distance_mode: profile.config.long_distance_mode,
        }),
        is_default: profile.is_default,
    }
}

fn convert_proto_to_profile_config(proto: &MouseConfig) -> Result<ProfileConfig, Status> {
    let polling_rate = convert_polling_rate(PollingRate::try_from(proto.polling_rate).unwrap_or(PollingRate::Unspecified))?;
    let lod = convert_lift_off_distance(LiftOffDistance::try_from(proto.lift_off_distance).unwrap_or(LiftOffDistance::Unspecified))?;

    Ok(ProfileConfig {
        polling_rate_hz: polling_rate.to_hz(),
        lift_off_distance_mm: lod.to_mm(),
        sleep_timeout_seconds: proto.sleep_timeout_seconds as u16,
        angle_snapping: proto.angle_snapping,
        ripple_control: proto.ripple_control,
        high_speed_mode: proto.high_speed_mode,
        long_distance_mode: proto.long_distance_mode,
    })
}

fn profile_config_to_mouse_config(
    config: &ProfileConfig,
) -> Result<scyrox::MouseConfig, Status> {
    Ok(scyrox::MouseConfig {
        polling_rate: hz_to_scyrox_polling_rate(config.polling_rate_hz)?,
        lift_off_distance: mm_to_scyrox_lod(config.lift_off_distance_mm)?,
        sleep_timeout_seconds: config.sleep_timeout_seconds,
        angle_snapping: config.angle_snapping,
        ripple_control: config.ripple_control,
        high_speed_mode: config.high_speed_mode,
        long_distance_mode: config.long_distance_mode,
    })
}

fn hz_to_polling_rate(hz: u16) -> PollingRate {
    match hz {
        125 => PollingRate::PollingRate125,
        250 => PollingRate::PollingRate250,
        500 => PollingRate::PollingRate500,
        1000 => PollingRate::PollingRate1000,
        2000 => PollingRate::PollingRate2000,
        4000 => PollingRate::PollingRate4000,
        8000 => PollingRate::PollingRate8000,
        _ => PollingRate::Unspecified,
    }
}

fn hz_to_scyrox_polling_rate(hz: u16) -> Result<scyrox::PollingRate, Status> {
    match hz {
        125 => Ok(scyrox::PollingRate::Hz125),
        250 => Ok(scyrox::PollingRate::Hz250),
        500 => Ok(scyrox::PollingRate::Hz500),
        1000 => Ok(scyrox::PollingRate::Hz1000),
        2000 => Ok(scyrox::PollingRate::Hz2000),
        4000 => Ok(scyrox::PollingRate::Hz4000),
        8000 => Ok(scyrox::PollingRate::Hz8000),
        _ => Err(Status::invalid_argument(format!(
            "Invalid polling rate: {}",
            hz
        ))),
    }
}

fn mm_to_lift_off_distance(mm: f32) -> LiftOffDistance {
    if mm <= 0.85 {
        LiftOffDistance::Low
    } else if mm <= 1.5 {
        LiftOffDistance::Medium
    } else {
        LiftOffDistance::High
    }
}

fn mm_to_scyrox_lod(mm: f32) -> Result<scyrox::LiftOffDistance, Status> {
    if mm <= 0.85 {
        Ok(scyrox::LiftOffDistance::Low)
    } else if mm <= 1.5 {
        Ok(scyrox::LiftOffDistance::Medium)
    } else {
        Ok(scyrox::LiftOffDistance::High)
    }
}
