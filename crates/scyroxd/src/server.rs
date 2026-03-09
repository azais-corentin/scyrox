//! gRPC service implementation.

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use directories::ProjectDirs;
use tokio::sync::{Mutex, broadcast};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status};
use tracing::{debug, info, instrument, warn};

use scyrox::{Mouse, MouseError};
use scyrox_proto::{
    ApplyProfileRequest, BatteryStatus as ProtoBattery, BatteryUpdate, ConnectionChange,
    ConnectionMode, CreateProfileRequest, DaemonInfo, DeleteProfileRequest, DeviceStatus, Empty,
    Event, FirmwareInfo as ProtoFirmware, GetProfileRequest, LiftOffDistance as ProtoLod,
    PollingRate as ProtoRate, Profile, ProfileApplied, ProfileList, Scyrox, SetBoolRequest,
    SetDefaultProfileRequest, SetLiftOffDistanceRequest, SetPollingRateRequest,
    SetSleepTimeoutRequest, SetSleepTimeoutResponse, SettingsChanged, UpdateProfileRequest, event,
    hz_to_proto_polling_rate, mm_to_proto_lod,
};

use crate::config::DaemonConfig;
use crate::hotplug::DeviceEvent;
use crate::profiles::{ProfileConfig, ProfileStore};

/// Macro to wrap RPC handler logic with mouse locking and error handling.
///
/// The body should return `Result<T, scyrox::MouseError>`. The macro will:
/// 1. Ensure the mouse is connected
/// 2. Lock the mouse mutex
/// 3. Execute the body
/// 4. Convert MouseError to Status if needed
/// 5. Wrap success in Response::new()
macro_rules! with_mouse {
    ($self:expr, |$mouse:ident| $body:expr) => {{
        $self.ensure_mouse().await?;
        let guard = $self.mouse.lock().await;
        let $mouse = guard.as_ref().unwrap();
        #[allow(clippy::redundant_closure_call)]
        let result: ::std::result::Result<_, scyrox::MouseError> = (async { $body }).await;
        match result {
            Ok(val) => Ok(Response::new(val)),
            Err(e) => {
                drop(guard);
                Err($self.handle_mouse_error(e).await)
            }
        }
    }};
}

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
    /// Currently active profile ID (the profile last applied to the mouse).
    active_profile_id: Arc<Mutex<Option<String>>>,
    /// Shutdown signal sender.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    /// Sender for client events (watch_events subscribers).
    client_event_tx: broadcast::Sender<Event>,
}

impl ScyroxService {
    /// Create a new service instance.
    ///
    /// Returns the service, a shutdown receiver, and a device event receiver to be processed
    /// by a background task.
    pub async fn new(
        config: DaemonConfig,
        dirs: ProjectDirs,
        device_event_rx: broadcast::Receiver<DeviceEvent>,
    ) -> Result<(
        Self,
        tokio::sync::watch::Receiver<bool>,
        broadcast::Receiver<DeviceEvent>,
    )> {
        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Create client event broadcast channel
        let (client_event_tx, _) = broadcast::channel(32);

        let service = Self {
            mouse: Arc::new(Mutex::new(None)),
            profiles: ProfileStore::new(&dirs),
            config,
            start_time: Instant::now(),
            active_profile_id: Arc::new(Mutex::new(None)),
            shutdown_tx,
            client_event_tx,
        };

        match Mouse::open().await {
            Ok(m) => {
                info!("Mouse connected");
                service.spawn_notification_forwarder(&m);
                *service.mouse.lock().await = Some(m);
            }
            Err(e) => {
                warn!("Mouse not connected: {}", e);
            }
        }

        Ok((service, shutdown_rx, device_event_rx))
    }

    /// Ensure mouse is connected, attempting to reconnect if needed.
    async fn ensure_mouse(&self) -> Result<(), Status> {
        let mut guard = self.mouse.lock().await;
        if guard.is_none() {
            match Mouse::open().await {
                Ok(m) => {
                    info!("Mouse reconnected");
                    self.spawn_notification_forwarder(&m);
                    *guard = Some(m);
                }
                Err(_) => {
                    return Err(Status::unavailable(
                        "Mouse not connected. Please connect the device.",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Spawn a task to forward mouse notifications to clients.
    fn spawn_notification_forwarder(&self, mouse: &Mouse) {
        spawn_notification_forwarder(mouse, Arc::clone(&self.mouse), self.client_event_tx.clone());
    }

    /// Invalidate the current mouse connection.
    ///
    /// Called when we detect the device has been disconnected.
    pub async fn invalidate_mouse(&self) {
        let mut guard = self.mouse.lock().await;
        if guard.is_some() {
            debug!("invalidating mouse connection");
            *guard = None;
        }
    }

    /// Convert a mouse error to a gRPC Status using the error's display message directly.
    ///
    /// Returns a tuple of (Status, should_invalidate) where should_invalidate indicates
    /// the mouse connection should be dropped.
    fn mouse_error_to_status(e: scyrox::MouseError) -> (Status, bool) {
        let message = e.to_string();
        let should_invalidate = matches!(
            e,
            MouseError::Disconnected | MouseError::ChannelClosed | MouseError::TaskPanic
        );

        let status = match e {
            // Connection issues → unavailable
            MouseError::NotFound { .. }
            | MouseError::Disconnected
            | MouseError::DeviceOffline
            | MouseError::ChannelClosed => Status::unavailable(message),
            // Validation errors → invalid_argument
            MouseError::InvalidPollingRate(_)
            | MouseError::InvalidLiftOffDistance(_)
            | MouseError::InvalidSleepTimeout(_)
            | MouseError::InvalidDpiStage(_)
            | MouseError::InvalidDpiValue(_)
            | MouseError::InvalidDebounceTime(_)
            | MouseError::InvalidProfile(_) => Status::invalid_argument(message),
            // Protocol/communication errors → internal
            MouseError::Hid(_)
            | MouseError::Timeout
            | MouseError::UnexpectedResponse { .. }
            | MouseError::InsufficientData { .. }
            | MouseError::NotSupported
            | MouseError::TaskPanic => Status::internal(message),
        };

        (status, should_invalidate)
    }

    /// Handle a mouse error, invalidating the connection if needed.
    async fn handle_mouse_error(&self, e: scyrox::MouseError) -> Status {
        let (status, should_invalidate) = Self::mouse_error_to_status(e);

        if should_invalidate {
            self.invalidate_mouse().await;
            // Broadcast disconnection event
            let _ = self.client_event_tx.send(Event {
                event: Some(event::Event::ConnectionChange(ConnectionChange {
                    connected: false,
                    mode: ConnectionMode::Unspecified as i32,
                })),
            });
        }

        status
    }

    /// Create a device event handler that can be spawned as a background task.
    ///
    /// Returns a future that processes device events.
    pub fn create_device_event_handler(
        &self,
        mut rx: broadcast::Receiver<DeviceEvent>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let mouse = Arc::clone(&self.mouse);
        let active_profile_id = Arc::clone(&self.active_profile_id);
        let client_event_tx = self.client_event_tx.clone();
        let profiles = self.profiles.clone();
        let config = self.config.clone();

        async move {
            info!("device event handler started");

            while let Ok(event) = rx.recv().await {
                match event {
                    DeviceEvent::Connected { mode } => {
                        info!(?mode, "device connected");

                        match Mouse::open().await {
                            Ok(m) => {
                                spawn_notification_forwarder(
                                    &m,
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                );
                                let mut guard = mouse.lock().await;
                                *guard = Some(m);
                                info!("mouse connection established");

                                let proto_mode = match mode {
                                    scyrox::ConnectionMode::Wired => ConnectionMode::Wired,
                                    scyrox::ConnectionMode::Wireless => ConnectionMode::Wireless,
                                };
                                let _ = client_event_tx.send(Event {
                                    event: Some(event::Event::ConnectionChange(ConnectionChange {
                                        connected: true,
                                        mode: proto_mode as i32,
                                    })),
                                });

                                // Fetch and emit current battery on reconnect (with retry)
                                drop(guard);
                                spawn_battery_fetch_with_retry(
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                );

                                // Auto-apply last active profile or default
                                auto_apply_profile(
                                    &mouse,
                                    &active_profile_id,
                                    &client_event_tx,
                                    &profiles,
                                    &config,
                                )
                                .await;
                            }
                            Err(e) => {
                                warn!("failed to open mouse after connection event: {}", e);
                            }
                        }
                    }
                    DeviceEvent::Disconnected => {
                        info!("device disconnected");
                        {
                            let mut guard = mouse.lock().await;
                            if guard.is_some() {
                                debug!("invalidating mouse connection");
                                *guard = None;
                            }
                        }

                        let _ = client_event_tx.send(Event {
                            event: Some(event::Event::ConnectionChange(ConnectionChange {
                                connected: false,
                                mode: ConnectionMode::Unspecified as i32,
                            })),
                        });
                    }
                    DeviceEvent::ModeChanged { from, to } => {
                        info!(?from, ?to, "connection mode changed");
                        {
                            let mut guard = mouse.lock().await;
                            *guard = None;
                        }

                        match Mouse::open().await {
                            Ok(m) => {
                                spawn_notification_forwarder(
                                    &m,
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                );
                                let mut guard = mouse.lock().await;
                                *guard = Some(m);
                                info!(?to, "mouse reconnected in new mode");

                                let proto_mode = match to {
                                    scyrox::ConnectionMode::Wired => ConnectionMode::Wired,
                                    scyrox::ConnectionMode::Wireless => ConnectionMode::Wireless,
                                };
                                let _ = client_event_tx.send(Event {
                                    event: Some(event::Event::ConnectionChange(ConnectionChange {
                                        connected: true,
                                        mode: proto_mode as i32,
                                    })),
                                });

                                // Fetch and emit current battery on mode change (with retry)
                                drop(guard);
                                spawn_battery_fetch_with_retry(
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                );

                                // Re-apply last active profile
                                auto_apply_profile(
                                    &mouse,
                                    &active_profile_id,
                                    &client_event_tx,
                                    &profiles,
                                    &config,
                                )
                                .await;
                            }
                            Err(e) => {
                                warn!("failed to open mouse after mode change: {}", e);
                            }
                        }
                    }
                }
            }

            warn!("device event receiver closed");
        }
    }
}

/// Spawn a task to forward mouse notifications to clients (static version for use in closures).
fn spawn_notification_forwarder(
    mouse: &Mouse,
    mouse_arc: Arc<Mutex<Option<Mouse>>>,
    client_event_tx: broadcast::Sender<Event>,
) {
    let notification_rx = mouse.subscribe_notifications();

    tokio::spawn(async move {
        let mut rx = notification_rx;
        loop {
            match rx.recv().await {
                Ok(notif) => match notif {
                    scyrox::Notification::StatusChanged(flags) => {
                        let _ = client_event_tx.send(Event {
                            event: Some(event::Event::SettingsChanged(SettingsChanged {
                                dpi_changed: flags.dpi_changed(),
                                report_rate_changed: flags.report_rate_changed(),
                                profile_changed: flags.profile_changed(),
                                dpi_settings_changed: flags.dpi_settings_changed(),
                                light_settings_changed: flags.light_settings_changed(),
                                battery_changed: flags.battery_changed(),
                            })),
                        });

                        if flags.battery_changed() {
                            let mouse_arc = mouse_arc.clone();
                            let tx = client_event_tx.clone();
                            tokio::spawn(async move {
                                let guard = mouse_arc.lock().await;
                                if let Some(m) = guard.as_ref() {
                                    match m.get_battery().await {
                                        Ok(battery) => {
                                            let _ = tx.send(Event {
                                                event: Some(event::Event::BatteryUpdate(
                                                    BatteryUpdate {
                                                        status: Some(ProtoBattery {
                                                            voltage_mv: battery.voltage_mv as u32,
                                                            percentage: battery.percentage as u32,
                                                            charging: battery.charging,
                                                        }),
                                                    },
                                                )),
                                            });
                                        }
                                        Err(e) => {
                                            warn!(
                                                "failed to fetch battery after change notification: {e}"
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    }
                    scyrox::Notification::Disconnected => {
                        let _ = client_event_tx.send(Event {
                            event: Some(event::Event::ConnectionChange(ConnectionChange {
                                connected: false,
                                mode: ConnectionMode::Unspecified as i32,
                            })),
                        });
                        break;
                    }
                },
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "notification forwarder lagged, continuing");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("notification channel closed, forwarder exiting");
                    break;
                }
            }
        }
    });
}

/// Spawn a task that fetches battery status with retries after reconnection.
///
/// USB devices often aren't ready for HID commands immediately after hotplug
/// enumeration, so we wait briefly and retry up to 3 times.
fn spawn_battery_fetch_with_retry(mouse: Arc<Mutex<Option<Mouse>>>, tx: broadcast::Sender<Event>) {
    tokio::spawn(async move {
        // Give device time to become ready after USB enumeration
        tokio::time::sleep(Duration::from_millis(500)).await;

        for attempt in 1..=3 {
            let guard = mouse.lock().await;
            if let Some(m) = guard.as_ref() {
                match m.get_battery().await {
                    Ok(battery) => {
                        let _ = tx.send(Event {
                            event: Some(event::Event::BatteryUpdate(BatteryUpdate {
                                status: Some(ProtoBattery {
                                    voltage_mv: battery.voltage_mv as u32,
                                    percentage: battery.percentage as u32,
                                    charging: battery.charging,
                                }),
                            })),
                        });
                        return;
                    }
                    Err(e) => {
                        warn!(attempt, "failed to fetch battery after reconnect: {e}");
                        drop(guard);
                        if attempt < 3 {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            } else {
                // Mouse disconnected again before we could fetch
                return;
            }
        }
    });
}

/// Auto-apply the last active profile or default profile on reconnection.
async fn auto_apply_profile(
    mouse: &Arc<Mutex<Option<Mouse>>>,
    active_profile_id: &Arc<Mutex<Option<String>>>,
    client_event_tx: &broadcast::Sender<Event>,
    profiles: &ProfileStore,
    config: &DaemonConfig,
) {
    if !config.auto_apply_on_connect {
        debug!("auto-apply disabled in config");
        return;
    }

    // First try the last active profile
    let profile_id = {
        let active = active_profile_id.lock().await;
        active.clone()
    };

    // Fall back to default profile from config
    let profile_id = profile_id.or_else(|| config.default_profile_id.clone());

    let Some(profile_id) = profile_id else {
        debug!("no profile to auto-apply");
        return;
    };

    // Load and apply the profile
    match profiles.get(&profile_id).await {
        Ok(profile) => {
            match profile_config_to_mouse_config(&profile.config) {
                Ok(mouse_config) => {
                    let guard = mouse.lock().await;
                    if let Some(m) = guard.as_ref() {
                        match m.set_config(&mouse_config).await {
                            Ok(()) => {
                                info!(
                                    profile_id = profile_id,
                                    profile_name = profile.name,
                                    "auto-applied profile"
                                );
                                // Update active profile ID
                                drop(guard);
                                let mut active = active_profile_id.lock().await;
                                *active = Some(profile_id.clone());

                                // Broadcast profile applied event
                                let _ = client_event_tx.send(Event {
                                    event: Some(event::Event::ProfileApplied(ProfileApplied {
                                        profile_id,
                                        profile_name: profile.name,
                                    })),
                                });
                            }
                            Err(e) => {
                                warn!("failed to auto-apply profile: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("failed to convert profile config: {}", e);
                }
            }
        }
        Err(e) => {
            warn!(profile_id = profile_id, "failed to load profile: {}", e);
        }
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
    ) -> Result<Response<ProtoBattery>, Status> {
        with_mouse!(self, |mouse| {
            let battery = mouse.get_battery().await?;
            Ok(ProtoBattery {
                voltage_mv: battery.voltage_mv as u32,
                percentage: battery.percentage as u32,
                charging: battery.charging,
            })
        })
    }

    #[instrument(skip(self, _request))]
    async fn get_firmware(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ProtoFirmware>, Status> {
        with_mouse!(self, |mouse| {
            let firmware = mouse.get_firmware_info().await?;
            Ok(ProtoFirmware {
                mouse_version: firmware.mouse_version,
                receiver_version: firmware.receiver_version,
            })
        })
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    #[instrument(skip(self, _request))]
    async fn get_config(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<scyrox_proto::MouseConfig>, Status> {
        with_mouse!(self, |mouse| {
            let config = mouse.get_config().await?;
            Ok(scyrox_proto::MouseConfig::from(&config))
        })
    }

    #[instrument(skip(self, request))]
    async fn set_config(
        &self,
        request: Request<scyrox_proto::MouseConfig>,
    ) -> Result<Response<Empty>, Status> {
        let proto_config = request.into_inner();
        let config = scyrox::MouseConfig::try_from(&proto_config)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        with_mouse!(self, |mouse| {
            mouse.set_config(&config).await?;
            info!("Configuration updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_polling_rate(
        &self,
        request: Request<SetPollingRateRequest>,
    ) -> Result<Response<Empty>, Status> {
        let rate = scyrox::PollingRate::try_from(request.into_inner().rate())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        with_mouse!(self, |mouse| {
            mouse.set_polling_rate(rate).await?;
            info!(?rate, "Polling rate updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_lift_off_distance(
        &self,
        request: Request<SetLiftOffDistanceRequest>,
    ) -> Result<Response<Empty>, Status> {
        let lod = scyrox::LiftOffDistance::try_from(request.into_inner().distance())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        with_mouse!(self, |mouse| {
            mouse.set_lift_off_distance(lod).await?;
            info!(?lod, "Lift-off distance updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_sleep_timeout(
        &self,
        request: Request<SetSleepTimeoutRequest>,
    ) -> Result<Response<SetSleepTimeoutResponse>, Status> {
        let seconds = request.into_inner().seconds as u16;

        with_mouse!(self, |mouse| {
            let actual_seconds = mouse.set_sleep_timeout(seconds).await?;
            info!(actual_seconds, "Sleep timeout updated");
            Ok(SetSleepTimeoutResponse {
                actual_seconds: actual_seconds as u32,
            })
        })
    }

    #[instrument(skip(self, request))]
    async fn set_angle_snapping(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        let enabled = request.into_inner().enabled;

        with_mouse!(self, |mouse| {
            mouse.set_angle_snapping(enabled).await?;
            info!(enabled, "Angle snapping updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_ripple_control(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        let enabled = request.into_inner().enabled;

        with_mouse!(self, |mouse| {
            mouse.set_ripple_control(enabled).await?;
            info!(enabled, "Ripple control updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_high_speed_mode(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        let enabled = request.into_inner().enabled;

        with_mouse!(self, |mouse| {
            mouse.set_high_speed_mode(enabled).await?;
            info!(enabled, "High speed mode updated");
            Ok(Empty {})
        })
    }

    #[instrument(skip(self, request))]
    async fn set_long_distance_mode(
        &self,
        request: Request<SetBoolRequest>,
    ) -> Result<Response<Empty>, Status> {
        let enabled = request.into_inner().enabled;

        with_mouse!(self, |mouse| {
            mouse.set_long_distance_mode(enabled).await?;
            info!(enabled, "Long distance mode updated");
            Ok(Empty {})
        })
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

        let proto_profiles = profiles.into_iter().map(profile_to_proto).collect();

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

        Ok(Response::new(profile_to_proto(profile)))
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

        let profile_config = proto_to_profile_config(&config)?;

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

        Ok(Response::new(profile_to_proto(profile)))
    }

    #[instrument(skip(self, request))]
    async fn update_profile(
        &self,
        request: Request<UpdateProfileRequest>,
    ) -> Result<Response<Profile>, Status> {
        let req = request.into_inner();

        let config = req
            .config
            .map(|c| proto_to_profile_config(&c))
            .transpose()?;

        let profile = self
            .profiles
            .update(&req.id, req.name, config)
            .await
            .map_err(|e| Status::internal(format!("Failed to update profile: {}", e)))?;

        Ok(Response::new(profile_to_proto(profile)))
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

        let guard = self.mouse.lock().await;
        let mouse = guard.as_ref().unwrap();

        if let Err(e) = mouse.set_config(&config).await {
            drop(guard);
            return Err(self.handle_mouse_error(e).await);
        }

        // Track the active profile ID
        drop(guard);
        {
            let mut active_profile = self.active_profile_id.lock().await;
            *active_profile = Some(id.clone());
        }

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
        let rx = self.client_event_tx.subscribe();

        // Convert broadcast receiver to a stream of Result<Event, Status>
        let stream = BroadcastStream::new(rx).filter_map(|result| match result {
            Ok(event) => Some(Ok(event)),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                warn!(skipped = n, "client lagged behind, skipped events");
                None
            }
        });

        Ok(Response::new(Box::pin(stream)))
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

        // Get the currently active profile ID
        let active_profile_id = self.active_profile_id.lock().await.clone();

        Ok(Response::new(DaemonInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            device_status: Some(DeviceStatus {
                connected,
                connection_mode: mode,
            }),
            active_profile_id,
        }))
    }

    #[instrument(skip(self, _request))]
    async fn shutdown(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        info!("Shutdown requested via RPC");
        // Signal shutdown to the main loop
        let _ = self.shutdown_tx.send(true);
        Ok(Response::new(Empty {}))
    }
}

// =============================================================================
// Conversion Helpers
// =============================================================================

/// Convert a profile (with Hz/mm fields) to proto format.
fn profile_to_proto(profile: crate::profiles::Profile) -> Profile {
    Profile {
        id: profile.id,
        name: profile.name,
        config: Some(scyrox_proto::MouseConfig {
            polling_rate: hz_to_proto_polling_rate(profile.config.polling_rate_hz) as i32,
            lift_off_distance: mm_to_proto_lod(profile.config.lift_off_distance_mm) as i32,
            sleep_timeout_seconds: profile.config.sleep_timeout_seconds as u32,
            angle_snapping: profile.config.angle_snapping,
            ripple_control: profile.config.ripple_control,
            high_speed_mode: profile.config.high_speed_mode,
            long_distance_mode: profile.config.long_distance_mode,
        }),
        is_default: profile.is_default,
    }
}

/// Convert proto MouseConfig to ProfileConfig (with Hz/mm fields).
fn proto_to_profile_config(proto: &scyrox_proto::MouseConfig) -> Result<ProfileConfig, Status> {
    let polling_rate = scyrox::PollingRate::try_from(
        ProtoRate::try_from(proto.polling_rate).unwrap_or(ProtoRate::Unspecified),
    )
    .map_err(|e| Status::invalid_argument(e.to_string()))?;

    let lod = scyrox::LiftOffDistance::try_from(
        ProtoLod::try_from(proto.lift_off_distance).unwrap_or(ProtoLod::Unspecified),
    )
    .map_err(|e| Status::invalid_argument(e.to_string()))?;

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

/// Convert ProfileConfig (with Hz/mm fields) to MouseConfig for the device.
fn profile_config_to_mouse_config(config: &ProfileConfig) -> Result<scyrox::MouseConfig, Status> {
    let polling_rate = scyrox::PollingRate::from_hz(config.polling_rate_hz).ok_or_else(|| {
        Status::invalid_argument(format!("Invalid polling rate: {}", config.polling_rate_hz))
    })?;

    let lift_off_distance = scyrox::LiftOffDistance::from_mm(config.lift_off_distance_mm)
        .unwrap_or(
            // Fall back to range-based matching for approximate values
            if config.lift_off_distance_mm <= 0.85 {
                scyrox::LiftOffDistance::Low
            } else if config.lift_off_distance_mm <= 1.5 {
                scyrox::LiftOffDistance::Medium
            } else {
                scyrox::LiftOffDistance::High
            },
        );

    Ok(scyrox::MouseConfig {
        polling_rate,
        lift_off_distance,
        sleep_timeout_seconds: config.sleep_timeout_seconds,
        angle_snapping: config.angle_snapping,
        ripple_control: config.ripple_control,
        high_speed_mode: config.high_speed_mode,
        long_distance_mode: config.long_distance_mode,
        ..Default::default()
    })
}
