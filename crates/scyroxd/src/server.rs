//! gRPC service implementation.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use directories::ProjectDirs;
use tokio::sync::{Mutex, broadcast, watch};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tonic::{Request, Response, Status};
use tracing::{debug, info, instrument, warn};

use scyrox::{BatteryStatus, Mouse, MouseError};
use scyrox_proto::{
    ApplyProfileRequest, BatteryStatus as ProtoBattery, BatteryUpdate, ConnectionChange,
    ConnectionMode, CreateProfileRequest, DaemonConfig as ProtoDaemonConfig, DaemonConfigChanged,
    DaemonInfo, DeleteProfileRequest, DeviceStatus, Empty, Event, FirmwareInfo as ProtoFirmware,
    GetProfileRequest, LiftOffDistance as ProtoLod, LowBatteryAlert, PollingRate as ProtoRate,
    Profile, ProfileApplied, ProfileList, Scyrox, SetBatteryLogPathRequest, SetBoolRequest,
    SetDefaultProfileRequest, SetLiftOffDistanceRequest, SetLowBatteryThresholdRequest,
    SetPollingRateRequest, SetSleepTimeoutRequest, SetSleepTimeoutResponse, SettingsChanged,
    UpdateProfileRequest, event, hz_to_proto_polling_rate, mm_to_proto_lod,
};

use crate::battery_log::{
    BatteryLifecycleSource, BatteryLogOpenError, BatteryLogger, BatteryRefreshSource,
    PreparedBatteryLogRecord,
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
    /// Shared daemon configuration.
    config: Arc<Mutex<DaemonConfig>>,
    /// Path used to persist daemon configuration.
    config_path: PathBuf,
    /// Directory used to resolve relative daemon configuration paths.
    config_dir: PathBuf,
    /// Append-only battery observation logger.
    battery_logger: BatteryLogger,
    /// Daemon start time.
    start_time: Instant,
    /// Currently active profile ID (the profile last applied to the mouse).
    active_profile_id: Arc<Mutex<Option<String>>>,
    /// Shutdown signal sender.
    shutdown_tx: watch::Sender<bool>,
    /// Sender for client events (watch_events subscribers).
    client_event_tx: broadcast::Sender<Event>,
}

impl ScyroxService {
    /// Create a new service instance.
    ///
    /// Returns the service and the device event receiver to be processed by a background task.
    pub async fn new(
        config: DaemonConfig,
        dirs: ProjectDirs,
        device_event_rx: broadcast::Receiver<DeviceEvent>,
        shutdown_tx: watch::Sender<bool>,
    ) -> Result<(Self, broadcast::Receiver<DeviceEvent>)> {
        // Create client event broadcast channel
        let (client_event_tx, _) = broadcast::channel(32);

        let config_path = DaemonConfig::path(&dirs);
        let config_dir = dirs.config_dir().to_path_buf();
        let config_temp_path = config_path.with_extension("toml.tmp");
        let resolved_battery_log_path = config.resolved_battery_log_path(&config_dir);
        let battery_logger = BatteryLogger::new(
            resolved_battery_log_path.as_deref(),
            &[&config_path, &config_temp_path],
        )
        .await?;
        let service = Self {
            mouse: Arc::new(Mutex::new(None)),
            profiles: ProfileStore::new(&dirs),
            config: Arc::new(Mutex::new(config)),
            config_path,
            config_dir,
            battery_logger,
            start_time: Instant::now(),
            active_profile_id: Arc::new(Mutex::new(None)),
            shutdown_tx,
            client_event_tx,
        };

        match Mouse::open().await {
            Ok(m) => {
                info!("Mouse connected");
                service
                    .battery_logger
                    .log_device_connected(BatteryLifecycleSource::Startup, m.connection_mode())
                    .await;
                service.spawn_notification_forwarder(&m);
                *service.mouse.lock().await = Some(m);
            }
            Err(e) => {
                warn!("Mouse not connected: {}", e);
            }
        }

        Ok((service, device_event_rx))
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
        spawn_notification_forwarder(
            mouse,
            Arc::clone(&self.mouse),
            self.client_event_tx.clone(),
            self.battery_logger.clone(),
        );
    }

    /// Invalidate the current mouse connection.
    ///
    /// Called when we detect the device has been disconnected.
    pub async fn invalidate_mouse(&self) {
        let mut guard = self.mouse.lock().await;
        if guard.is_some() {
            debug!("invalidating mouse connection");
        }
        dispose_mouse(&mut guard).await;
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
        let battery_logger = self.battery_logger.clone();

        async move {
            info!("device event handler started");

            loop {
                let event = match rx.recv().await {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "device event handler lagged, continuing");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                match event {
                    DeviceEvent::Connected { mode } => {
                        battery_logger
                            .log_device_connected(BatteryLifecycleSource::Hotplug, mode)
                            .await;
                        info!(?mode, "device connected");

                        match Mouse::open().await {
                            Ok(m) => {
                                spawn_notification_forwarder(
                                    &m,
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                    battery_logger.clone(),
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
                                    battery_logger.clone(),
                                    BatteryRefreshSource::DeviceConnected,
                                );

                                let config_snapshot = config.lock().await.clone();
                                // Auto-apply last active profile or default
                                auto_apply_profile(
                                    &mouse,
                                    &active_profile_id,
                                    &client_event_tx,
                                    &profiles,
                                    &config_snapshot,
                                )
                                .await;
                            }
                            Err(e) => {
                                warn!("failed to open mouse after connection event: {}", e);
                            }
                        }
                    }
                    DeviceEvent::Disconnected => {
                        battery_logger
                            .log_device_disconnected(BatteryLifecycleSource::Hotplug)
                            .await;
                        info!("device disconnected");
                        {
                            let mut guard = mouse.lock().await;
                            if guard.is_some() {
                                debug!("invalidating mouse connection");
                            }
                            dispose_mouse(&mut guard).await;
                        }

                        let _ = client_event_tx.send(Event {
                            event: Some(event::Event::ConnectionChange(ConnectionChange {
                                connected: false,
                                mode: ConnectionMode::Unspecified as i32,
                            })),
                        });
                    }
                    DeviceEvent::ModeChanged { from, to } => {
                        battery_logger
                            .log_connection_mode_changed(BatteryLifecycleSource::Hotplug, from, to)
                            .await;
                        info!(?from, ?to, "connection mode changed");
                        {
                            let mut guard = mouse.lock().await;
                            dispose_mouse(&mut guard).await;
                        }

                        match Mouse::open().await {
                            Ok(m) => {
                                spawn_notification_forwarder(
                                    &m,
                                    Arc::clone(&mouse),
                                    client_event_tx.clone(),
                                    battery_logger.clone(),
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
                                    battery_logger.clone(),
                                    BatteryRefreshSource::ModeChanged,
                                );

                                let config_snapshot = config.lock().await.clone();
                                // Re-apply last active profile
                                auto_apply_profile(
                                    &mouse,
                                    &active_profile_id,
                                    &client_event_tx,
                                    &profiles,
                                    &config_snapshot,
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

    /// Create a periodic battery poll task.
    ///
    /// Broadcasts a `BatteryUpdate` every `battery_poll_interval_secs` while the
    /// mouse is connected and awake. A `LowBatteryAlert` fires once at or below
    /// the live daemon-owned threshold while discharging, then re-arms after
    /// charging or five percentage points of recovery. An interval of 0 disables
    /// polling.
    pub fn create_battery_poll_task(&self) -> impl Future<Output = ()> + Send + 'static {
        let mouse = Arc::clone(&self.mouse);
        let client_event_tx = self.client_event_tx.clone();
        let config = Arc::clone(&self.config);
        let battery_logger = self.battery_logger.clone();

        async move {
            let interval_secs = config.lock().await.battery_poll_interval_secs;
            if interval_secs == 0 {
                info!("battery polling disabled (battery_poll_interval_secs = 0)");
                return;
            }
            info!(interval_secs, "battery poll task started");

            let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Consume the immediate first tick: startup/reconnect fetches are
            // already covered by spawn_battery_fetch_with_retry and client RPCs.
            ticker.tick().await;

            let mut alerted = false;
            loop {
                ticker.tick().await;
                let threshold = config.lock().await.low_battery_threshold;

                let Some(ObservedBatteryRefresh { result, log_record }) = observe_battery_refresh(
                    &mouse,
                    &battery_logger,
                    BatteryRefreshSource::Periodic,
                    None,
                )
                .await
                else {
                    debug!("battery poll skipped: mouse disconnected");
                    continue;
                };
                if let Some(record) = log_record {
                    battery_logger.write_record(record).await;
                }

                match result {
                    Ok(battery) => {
                        debug!(
                            percentage = battery.percentage,
                            charging = battery.charging,
                            "periodic battery update"
                        );
                        let _ = client_event_tx.send(battery_update_event(&battery));

                        let (new_alerted, emit) = low_battery_transition(
                            alerted,
                            battery.percentage,
                            battery.charging,
                            threshold,
                        );
                        alerted = new_alerted;
                        if emit {
                            info!(
                                percentage = battery.percentage,
                                threshold, "low battery alert"
                            );
                            let _ = client_event_tx.send(Event {
                                event: Some(event::Event::LowBatteryAlert(LowBatteryAlert {
                                    percentage: battery.percentage as u32,
                                })),
                            });
                        }
                    }
                    Err(MouseError::DeviceOffline) => {
                        debug!("battery poll skipped: mouse sleeping/out of range");
                    }
                    Err(e) => {
                        warn!("periodic battery poll failed: {e}");
                    }
                }
            }
        }
    }
}

struct ObservedBatteryRefresh {
    result: scyrox::Result<BatteryStatus>,
    log_record: Option<PreparedBatteryLogRecord>,
}

async fn observe_battery_refresh(
    mouse: &Arc<Mutex<Option<Mouse>>>,
    logger: &BatteryLogger,
    source: BatteryRefreshSource,
    attempt: Option<u8>,
) -> Option<ObservedBatteryRefresh> {
    let sample = {
        let guard = mouse.lock().await;
        let mouse = guard.as_ref()?;
        mouse.get_battery_sample().await
    };

    Some(match sample {
        Ok(sample) => {
            let log_record = logger.prepare_sample_record(source, attempt, &sample);
            ObservedBatteryRefresh {
                result: Ok(sample.status),
                log_record,
            }
        }
        Err(error) => {
            let log_record = logger.prepare_refresh_error_record(source, attempt, &error);
            ObservedBatteryRefresh {
                result: Err(error),
                log_record,
            }
        }
    })
}

/// Take and gracefully shut down the current mouse connection, if any.
async fn dispose_mouse(guard: &mut Option<Mouse>) {
    if let Some(mouse) = guard.take()
        && let Err(e) = mouse.shutdown().await
    {
        warn!("mouse IO task shutdown failed: {e}");
    }
}

/// Build a `BatteryUpdate` broadcast event from a domain battery status.
fn battery_update_event(battery: &scyrox::BatteryStatus) -> Event {
    Event {
        event: Some(event::Event::BatteryUpdate(BatteryUpdate {
            status: Some(ProtoBattery {
                voltage_mv: battery.voltage_mv as u32,
                percentage: battery.percentage as u32,
                charging: battery.charging,
            }),
        })),
    }
}

/// Edge-triggered low-battery alert decision.
///
/// Returns `(new_alerted, emit_alert)`. An alert fires once at or below
/// `threshold` while discharging. Charging re-arms immediately; otherwise the
/// latch clears only when the percentage is above `threshold` and reaches
/// `min(threshold + 5, 100)`.
fn low_battery_transition(
    alerted: bool,
    percentage: u8,
    charging: bool,
    threshold: u8,
) -> (bool, bool) {
    if charging {
        return (false, false);
    }

    if alerted {
        let recovery_threshold = threshold.saturating_add(5).min(100);
        let recovered = percentage > threshold && percentage >= recovery_threshold;
        return (!recovered, false);
    }

    let emit = percentage <= threshold;
    (emit, emit)
}

/// Spawn a task to forward mouse notifications to clients (static version for use in closures).
fn spawn_notification_forwarder(
    mouse: &Mouse,
    mouse_arc: Arc<Mutex<Option<Mouse>>>,
    client_event_tx: broadcast::Sender<Event>,
    battery_logger: BatteryLogger,
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
                            let battery_logger = battery_logger.clone();
                            tokio::spawn(async move {
                                let Some(ObservedBatteryRefresh { result, log_record }) =
                                    observe_battery_refresh(
                                        &mouse_arc,
                                        &battery_logger,
                                        BatteryRefreshSource::BatteryChanged,
                                        None,
                                    )
                                    .await
                                else {
                                    return;
                                };
                                if let Some(record) = log_record {
                                    battery_logger.write_record(record).await;
                                }
                                match result {
                                    Ok(battery) => {
                                        let _ = tx.send(battery_update_event(&battery));
                                    }
                                    Err(error) => {
                                        warn!(
                                            "failed to fetch battery after change notification: {error}"
                                        );
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
fn spawn_battery_fetch_with_retry(
    mouse: Arc<Mutex<Option<Mouse>>>,
    tx: broadcast::Sender<Event>,
    battery_logger: BatteryLogger,
    source: BatteryRefreshSource,
) {
    tokio::spawn(async move {
        // Give device time to become ready after USB enumeration
        tokio::time::sleep(Duration::from_millis(500)).await;

        for attempt in 1..=3 {
            let Some(ObservedBatteryRefresh { result, log_record }) =
                observe_battery_refresh(&mouse, &battery_logger, source, Some(attempt)).await
            else {
                // Mouse disconnected again before we could fetch
                return;
            };
            if let Some(record) = log_record {
                battery_logger.write_record(record).await;
            }
            match result {
                Ok(battery) => {
                    let _ = tx.send(battery_update_event(&battery));
                    return;
                }
                Err(error) => {
                    warn!(attempt, "failed to fetch battery after reconnect: {error}");
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
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
        self.ensure_mouse().await?;
        let Some(ObservedBatteryRefresh { result, log_record }) = observe_battery_refresh(
            &self.mouse,
            &self.battery_logger,
            BatteryRefreshSource::Rpc,
            None,
        )
        .await
        else {
            return Err(Status::unavailable(
                "Mouse not connected. Please connect the device.",
            ));
        };

        match result {
            Ok(battery) => {
                if let Some(record) = log_record {
                    self.battery_logger.write_record(record).await;
                }
                Ok(Response::new(ProtoBattery {
                    voltage_mv: battery.voltage_mv as u32,
                    percentage: battery.percentage as u32,
                    charging: battery.charging,
                }))
            }
            Err(error) => {
                let status = self.handle_mouse_error(error).await;
                if let Some(record) = log_record {
                    self.battery_logger.write_record(record).await;
                }
                Err(status)
            }
        }
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
        let mut events = BroadcastStream::new(self.client_event_tx.subscribe());
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let stream = async_stream::stream! {
            loop {
                tokio::select! {
                    biased;

                    _ = async {
                        let _ = shutdown_rx.wait_for(|shutdown| *shutdown).await;
                    } => break,
                    event = events.next() => match event {
                        Some(Ok(event)) => yield Ok::<Event, Status>(event),
                        Some(Err(BroadcastStreamRecvError::Lagged(n))) => {
                            warn!(skipped = n, "client lagged behind, skipped events");
                        }
                        None => break,
                    },
                }
            }
        };

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
    async fn get_daemon_config(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ProtoDaemonConfig>, Status> {
        let config = self.config.lock().await;
        Ok(Response::new(daemon_config_to_proto(&config)))
    }

    #[instrument(skip(self, request))]
    async fn set_low_battery_threshold(
        &self,
        request: Request<SetLowBatteryThresholdRequest>,
    ) -> Result<Response<Empty>, Status> {
        let percentage = request.into_inner().percentage;
        if percentage > 100 {
            return Err(Status::invalid_argument(
                "low_battery_threshold must be between 0 and 100",
            ));
        }
        let percentage = percentage as u8;

        let mut config = self.config.lock().await;
        if config.low_battery_threshold == percentage {
            return Ok(Response::new(Empty {}));
        }

        let mut candidate = config.clone();
        candidate.low_battery_threshold = percentage;
        candidate
            .validate()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        candidate
            .save(&self.config_path)
            .await
            .map_err(|e| Status::internal(format!("Failed to save daemon configuration: {e}")))?;

        let proto_config = daemon_config_to_proto(&candidate);
        *config = candidate;

        // Send while still holding the lock so subscribers observe config
        // changes in commit order.
        let _ = self.client_event_tx.send(Event {
            event: Some(event::Event::DaemonConfigChanged(DaemonConfigChanged {
                config: Some(proto_config),
            })),
        });
        drop(config);

        Ok(Response::new(Empty {}))
    }

    #[instrument(skip(self, request))]
    async fn set_battery_log_path(
        &self,
        request: Request<SetBatteryLogPathRequest>,
    ) -> Result<Response<Empty>, Status> {
        let battery_log_path = request.into_inner().path.map(PathBuf::from);
        let mut config = self.config.lock().await;
        if config.battery_log_path == battery_log_path {
            return Ok(Response::new(Empty {}));
        }

        let mut candidate = config.clone();
        candidate.battery_log_path = battery_log_path;
        candidate
            .validate()
            .map_err(|error| Status::invalid_argument(error.to_string()))?;

        let resolved_path = candidate.resolved_battery_log_path(&self.config_dir);
        let config_temp_path = self.config_path.with_extension("toml.tmp");
        let prepared = self
            .battery_logger
            .prepare(
                resolved_path.as_deref(),
                &[&self.config_path, &config_temp_path],
            )
            .await
            .map_err(|error| match error {
                BatteryLogOpenError::ReservedConfigPath => {
                    Status::invalid_argument(error.to_string())
                }
                BatteryLogOpenError::Io { .. } => {
                    Status::internal(format!("Failed to open battery log: {error}"))
                }
            })?;

        candidate.save(&self.config_path).await.map_err(|error| {
            Status::internal(format!("Failed to save daemon configuration: {error}"))
        })?;

        let proto_config = daemon_config_to_proto(&candidate);
        self.battery_logger.replace(prepared).await;
        *config = candidate;
        let _ = self.client_event_tx.send(Event {
            event: Some(event::Event::DaemonConfigChanged(DaemonConfigChanged {
                config: Some(proto_config),
            })),
        });
        drop(config);

        Ok(Response::new(Empty {}))
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

fn daemon_config_to_proto(config: &DaemonConfig) -> ProtoDaemonConfig {
    ProtoDaemonConfig {
        low_battery_threshold: config.low_battery_threshold as u32,
        battery_log_path: config
            .battery_log_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    }
}

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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scyroxd-server-tests-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn config_path(&self) -> PathBuf {
            self.path.join("daemon.toml")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    async fn service_fixture(low_battery_threshold: u8) -> Option<(ScyroxService, TestDir)> {
        let dirs = ProjectDirs::from("dev", "scyrox", "scyroxd-test")?;
        let test_dir = TestDir::new();
        let config_path = test_dir.config_path();
        let config_dir = test_dir.path.clone();
        let battery_logger = BatteryLogger::new(None, &[]).await.unwrap();
        let (client_event_tx, _) = broadcast::channel(32);
        let service = ScyroxService {
            mouse: Arc::new(Mutex::new(None)),
            profiles: ProfileStore::new(&dirs),
            config: Arc::new(Mutex::new(DaemonConfig {
                low_battery_threshold,
                ..DaemonConfig::default()
            })),
            config_path,
            config_dir,
            battery_logger,
            start_time: Instant::now(),
            active_profile_id: Arc::new(Mutex::new(None)),
            shutdown_tx: tokio::sync::watch::channel(false).0,
            client_event_tx,
        };
        Some((service, test_dir))
    }
    #[tokio::test]
    async fn watch_events_ends_when_shutdown_is_requested() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let mut stream = service
            .watch_events(Request::new(Empty {}))
            .await
            .unwrap()
            .into_inner();

        service.shutdown(Request::new(Empty {})).await.unwrap();

        let next_event = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("event stream did not end after shutdown");
        assert!(next_event.is_none(), "event stream yielded after shutdown");
    }

    async fn get_low_battery_threshold(service: &ScyroxService) -> u32 {
        service
            .get_daemon_config(Request::new(Empty {}))
            .await
            .unwrap()
            .into_inner()
            .low_battery_threshold
    }

    /// The device-event handler must survive a `RecvError::Lagged` and keep
    /// processing subsequent events.
    #[tokio::test]
    async fn device_event_handler_survives_lagged() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };

        // Capacity-1 channel with two queued sends before the handler polls:
        // the first `recv()` observes `Lagged(1)`, the second yields the
        // retained `Disconnected`.
        let (tx, rx) = broadcast::channel(1);
        tx.send(DeviceEvent::Disconnected).unwrap();
        tx.send(DeviceEvent::Disconnected).unwrap();
        let mut client_rx = service.client_event_tx.subscribe();

        tokio::spawn(service.create_device_event_handler(rx));

        let event = tokio::time::timeout(Duration::from_secs(1), client_rx.recv())
            .await
            .expect("device event handler died on RecvError::Lagged")
            .unwrap();

        match event.event {
            Some(event::Event::ConnectionChange(change)) => assert!(!change.connected),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_daemon_config_returns_threshold_without_mouse() {
        let Some((service, _test_dir)) = service_fixture(37).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };

        assert_eq!(get_low_battery_threshold(&service).await, 37);
    }

    #[tokio::test]
    async fn set_low_battery_threshold_persists_updates_and_broadcasts() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let mut events = service.client_event_tx.subscribe();

        service
            .set_low_battery_threshold(Request::new(SetLowBatteryThresholdRequest {
                percentage: 17,
            }))
            .await
            .unwrap();

        assert_eq!(get_low_battery_threshold(&service).await, 17);
        let contents = tokio::fs::read_to_string(&service.config_path)
            .await
            .unwrap();
        let persisted: DaemonConfig = toml::from_str(&contents).unwrap();
        assert_eq!(persisted.low_battery_threshold, 17);

        let event = events.try_recv().unwrap();
        match event.event {
            Some(event::Event::DaemonConfigChanged(change)) => {
                assert_eq!(change.config.unwrap().low_battery_threshold, 17);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn setting_current_low_battery_threshold_is_noop() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let mut events = service.client_event_tx.subscribe();

        service
            .set_low_battery_threshold(Request::new(SetLowBatteryThresholdRequest {
                percentage: 10,
            }))
            .await
            .unwrap();

        assert!(!service.config_path.exists());
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn invalid_low_battery_threshold_does_not_mutate_or_broadcast() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let mut events = service.client_event_tx.subscribe();

        let status = service
            .set_low_battery_threshold(Request::new(SetLowBatteryThresholdRequest {
                percentage: 101,
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert_eq!(
            status.message(),
            "low_battery_threshold must be between 0 and 100"
        );
        assert_eq!(get_low_battery_threshold(&service).await, 10);
        assert!(!service.config_path.exists());
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn persistence_failure_keeps_previous_threshold_and_suppresses_event() {
        let Some((mut service, test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        service.config_path = test_dir.path.clone();
        let mut events = service.client_event_tx.subscribe();

        let status = service
            .set_low_battery_threshold(Request::new(SetLowBatteryThresholdRequest {
                percentage: 17,
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(
            status
                .message()
                .starts_with("Failed to save daemon configuration:")
        );
        assert_eq!(get_low_battery_threshold(&service).await, 10);
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn battery_log_path_setting_is_live_persistent_and_reversible() {
        let Some((service, test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let configured_path = PathBuf::from("captures/battery.jsonl");
        let resolved_path = test_dir.path.join(&configured_path);
        let mut events = service.client_event_tx.subscribe();

        service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some(configured_path.to_string_lossy().into_owned()),
            }))
            .await
            .unwrap();

        assert_eq!(
            service.config.lock().await.battery_log_path,
            Some(configured_path.clone())
        );
        let persisted: DaemonConfig = toml::from_str(
            &tokio::fs::read_to_string(&service.config_path)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.battery_log_path, Some(configured_path.clone()));
        let event = events.try_recv().unwrap();
        let event_path = match event.event {
            Some(event::Event::DaemonConfigChanged(change)) => {
                change.config.unwrap().battery_log_path
            }
            other => panic!("unexpected event: {other:?}"),
        };
        assert_eq!(event_path, Some("captures/battery.jsonl".to_owned()));
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));

        let sample = scyrox::BatterySample {
            status: scyrox::BatteryStatus {
                voltage_mv: 3700,
                percentage: 42,
                charging: false,
            },
            device_percentage: 41,
            raw_response: vec![0x08, 0x04, 0x00],
        };
        let record = service
            .battery_logger
            .prepare_sample_record(crate::battery_log::BatteryRefreshSource::Rpc, None, &sample)
            .unwrap();
        service.battery_logger.write_record(record).await;
        assert_eq!(
            tokio::fs::read_to_string(&resolved_path)
                .await
                .unwrap()
                .lines()
                .count(),
            1
        );

        service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest { path: None }))
            .await
            .unwrap();

        assert_eq!(service.config.lock().await.battery_log_path, None);
        assert!(
            service
                .battery_logger
                .prepare_sample_record(
                    crate::battery_log::BatteryRefreshSource::Rpc,
                    None,
                    &sample,
                )
                .is_none()
        );
        assert_eq!(
            tokio::fs::read_to_string(&resolved_path)
                .await
                .unwrap()
                .lines()
                .count(),
            1
        );
        let event = events.try_recv().unwrap();
        match event.event {
            Some(event::Event::DaemonConfigChanged(change)) => {
                assert_eq!(change.config.unwrap().battery_log_path, None);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn battery_log_path_directory_target_rolls_back() {
        let Some((service, test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let mut events = service.client_event_tx.subscribe();

        let status = service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some(test_dir.path.to_string_lossy().into_owned()),
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), tonic::Code::Internal);
        assert_eq!(service.config.lock().await.battery_log_path, None);
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn battery_log_path_reserved_alias_is_invalid_argument() {
        let Some((service, _test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };

        let status = service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some(service.config_path.to_string_lossy().into_owned()),
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert_eq!(
            status.message(),
            "battery_log_path must not alias daemon.toml or daemon.toml.tmp"
        );
        assert_eq!(service.config.lock().await.battery_log_path, None);
    }

    #[tokio::test]
    async fn battery_log_path_persistence_failure_keeps_old_live_sink() {
        let Some((service, test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        let old_path = test_dir.path.join("old.jsonl");
        let new_path = test_dir.path.join("new.jsonl");
        let mut events = service.client_event_tx.subscribe();
        service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some("old.jsonl".to_owned()),
            }))
            .await
            .unwrap();
        let _ = events.try_recv().unwrap();

        tokio::fs::remove_file(&service.config_path).await.unwrap();
        tokio::fs::create_dir(&service.config_path).await.unwrap();
        let status = service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some("new.jsonl".to_owned()),
            }))
            .await
            .unwrap_err();

        assert_eq!(status.code(), tonic::Code::Internal);
        assert_eq!(
            service.config.lock().await.battery_log_path,
            Some(PathBuf::from("old.jsonl"))
        );
        let sample = scyrox::BatterySample {
            status: scyrox::BatteryStatus {
                voltage_mv: 3700,
                percentage: 42,
                charging: false,
            },
            device_percentage: 41,
            raw_response: vec![0x08, 0x04, 0x00],
        };
        let record = service
            .battery_logger
            .prepare_sample_record(crate::battery_log::BatteryRefreshSource::Rpc, None, &sample)
            .unwrap();
        service.battery_logger.write_record(record).await;

        assert_eq!(
            tokio::fs::read_to_string(old_path)
                .await
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert_eq!(tokio::fs::read_to_string(new_path).await.unwrap(), "");
        assert!(matches!(
            events.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn battery_log_path_observer_skips_empty_mouse_slot_without_record() {
        let Some((service, test_dir)) = service_fixture(10).await else {
            eprintln!("skipping: ProjectDirs unavailable in this environment");
            return;
        };
        service
            .set_battery_log_path(Request::new(SetBatteryLogPathRequest {
                path: Some("battery.jsonl".to_owned()),
            }))
            .await
            .unwrap();

        let observed = observe_battery_refresh(
            &service.mouse,
            &service.battery_logger,
            BatteryRefreshSource::Rpc,
            None,
        )
        .await;

        assert!(observed.is_none());
        assert_eq!(
            tokio::fs::read_to_string(test_dir.path.join("battery.jsonl"))
                .await
                .unwrap(),
            ""
        );
    }

    #[test]
    fn low_battery_alert_fires_once_until_hysteresis_rearms() {
        let mut alerted = false;
        let mut emitted = Vec::new();

        for percentage in [10, 11, 9, 14, 15, 10] {
            let (new_alerted, emit) = low_battery_transition(alerted, percentage, false, 10);
            alerted = new_alerted;
            emitted.push(emit);
        }

        assert_eq!(emitted, [true, false, false, false, false, true]);
    }

    #[test]
    fn low_battery_alert_never_fires_while_charging() {
        assert_eq!(low_battery_transition(false, 5, true, 10), (false, false));
        assert_eq!(low_battery_transition(true, 5, true, 10), (false, false));
    }

    #[test]
    fn low_battery_alert_does_not_fire_just_above_threshold() {
        assert_eq!(low_battery_transition(false, 11, false, 10), (false, false));
    }

    #[test]
    fn high_threshold_hysteresis_caps_recovery_at_one_hundred() {
        assert_eq!(low_battery_transition(true, 100, false, 99), (false, false));
        assert_eq!(low_battery_transition(true, 100, false, 100), (true, false));
        assert_eq!(low_battery_transition(true, 100, true, 100), (false, false));
    }
}
