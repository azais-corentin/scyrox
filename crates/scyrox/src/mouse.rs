//! Mouse communication and configuration API.

use nusb::transfer::{In, Interrupt};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::error::{MouseError, Result};
use crate::io::{Command, IoTask};
use crate::protocol::*;
use crate::types::*;

/// Maximum number of key function slots.
pub const KEY_FUNCTION_COUNT: usize = 8;

/// Maximum number of shortcut key slots.
pub const SHORTCUT_KEY_COUNT: usize = 8;

/// Maximum number of macro slots.
pub const MACRO_COUNT: usize = 8;

/// Maximum sleep timeout in seconds (0xFF * 10).
pub const MAX_SLEEP_TIMEOUT_SECONDS: u16 = 2550;

/// Channel capacity for pending commands.
///
/// 16 allows a reasonable queue of commands while preventing unbounded memory
/// growth. In practice, commands are sent sequentially with responses awaited,
/// so this capacity is rarely reached. The main use case is allowing multiple
/// concurrent callers to queue commands without blocking.
const COMMAND_CHANNEL_CAPACITY: usize = 16;

/// Broadcast channel capacity for device notifications.
///
/// 32 provides buffer for burst notifications (e.g., rapid DPI button presses)
/// while keeping memory bounded. Receivers that fall behind by more than 32
/// messages will receive a `RecvError::Lagged` error and must handle it
/// appropriately (typically by continuing to receive future messages).
const NOTIFICATION_CHANNEL_CAPACITY: usize = 32;

/// Mouse device handle for async communication.
///
/// The `Mouse` struct owns a background IO task that handles all USB communication.
/// When dropped, the IO task will be signaled to shut down via channel closure.
/// For explicit control over shutdown, use the [`shutdown`](Self::shutdown) method.
pub struct Mouse {
    mode: ConnectionMode,
    command_tx: mpsc::Sender<Command>,
    notification_tx: broadcast::Sender<Notification>,
    task_handle: tokio::task::JoinHandle<()>,
}

/// Macro to implement getter and setter for boolean settings stored in memory.
///
/// This macro generates a pair of async methods for getting and setting a boolean
/// setting that is stored as a single byte in mouse memory (0x01 = enabled, 0x00 = disabled).
macro_rules! impl_bool_setting {
    ($get:ident, $set:ident, $offset:ident, $name:literal) => {
        #[doc = concat!("Get ", $name, " state.")]
        #[instrument(skip(self))]
        pub async fn $get(&self) -> Result<bool> {
            debug!("getting {}", $name);
            let byte = self.read_memory_byte($offset).await?;
            let enabled = byte == 0x01;
            debug!(enabled = enabled, "{} state retrieved", $name);
            Ok(enabled)
        }

        #[doc = concat!("Set ", $name, " state.")]
        #[instrument(skip(self))]
        pub async fn $set(&self, enabled: bool) -> Result<()> {
            info!(enabled = enabled, "setting {}", $name);
            self.write_memory($offset, if enabled { 0x01 } else { 0x00 })
                .await?;
            info!(enabled = enabled, "{} set successfully", $name);
            Ok(())
        }
    };
}

impl Mouse {
    /// Open a connection to the mouse.
    ///
    /// This spawns a background IO task that handles USB communication.
    /// The task runs until the device disconnects or the Mouse handle is dropped.
    #[instrument]
    pub async fn open() -> Result<Self> {
        debug!("searching for mouse device");

        let mut device_info = None;
        let mut mode = ConnectionMode::Wireless;

        for pid in PRODUCT_IDS {
            if let Ok(mut devices) = nusb::list_devices().await {
                if let Some(dev) =
                    devices.find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == pid)
                {
                    mode = match pid {
                        PID_WIRED => ConnectionMode::Wired,
                        PID_WIRELESS_4K | PID_WIRELESS_STD => ConnectionMode::Wireless,
                        _ => ConnectionMode::Wireless,
                    };
                    device_info = Some(dev);
                    break;
                }
            }
        }

        let device_info = device_info.ok_or_else(|| {
            error!(vid = VENDOR_ID, ?PRODUCT_IDS, "mouse device not found");
            MouseError::NotFound {
                vid: VENDOR_ID,
                pids: PRODUCT_IDS.to_vec(),
            }
        })?;

        debug!(?mode, "found mouse device");

        let device = device_info.open().await?;
        let interface = device.detach_and_claim_interface(INTERFACE_NUM).await?;
        let endpoint = interface.endpoint::<Interrupt, In>(INTERRUPT_EP_IN)?;

        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (notification_tx, _) = broadcast::channel(NOTIFICATION_CHANNEL_CAPACITY);

        let io_task = IoTask::new(
            interface,
            endpoint,
            mode,
            command_rx,
            notification_tx.clone(),
        );
        let task_handle = tokio::spawn(io_task.run());

        info!(?mode, "mouse connection established");

        Ok(Mouse {
            mode,
            command_tx,
            notification_tx,
            task_handle,
        })
    }

    /// Get the connection mode (wired or wireless).
    pub fn connection_mode(&self) -> ConnectionMode {
        trace!(mode = ?self.mode, "returning connection mode");
        self.mode
    }

    /// Subscribe to device notifications.
    ///
    /// Returns a receiver that will receive `Notification::StatusChanged` when the device
    /// sends unsolicited status updates, and `Notification::Disconnected` when the device
    /// is unplugged.
    pub fn subscribe_notifications(&self) -> broadcast::Receiver<Notification> {
        self.notification_tx.subscribe()
    }

    // =========================================================================
    // Lifecycle Management
    // =========================================================================

    /// Check if the IO task is still running.
    ///
    /// Returns `false` if the task has completed (due to disconnection, error,
    /// or shutdown). Note that this only checks if the task has finished; it
    /// does not probe the device for connectivity.
    pub fn is_running(&self) -> bool {
        !self.task_handle.is_finished()
    }

    /// Gracefully shut down the mouse connection.
    ///
    /// This closes the command channel, signals the IO task to terminate,
    /// and waits for it to complete. Any pending commands will receive a
    /// `ChannelClosed` error.
    ///
    /// This method consumes `self`, ensuring no further operations can be
    /// performed after shutdown.
    ///
    /// # Errors
    ///
    /// Returns `Err(MouseError::TaskPanic)` if the IO task panicked.
    #[instrument(skip(self))]
    pub async fn shutdown(self) -> Result<()> {
        info!("shutting down mouse connection");
        drop(self.command_tx);
        self.task_handle.await.map_err(|_| {
            error!("IO task panicked during shutdown");
            MouseError::TaskPanic
        })?;

        info!("mouse connection shut down successfully");
        Ok(())
    }

    /// Abort the IO task immediately without waiting.
    ///
    /// This forcefully terminates the IO task. Use [`shutdown`](Self::shutdown)
    /// for graceful termination. This is useful in scenarios where you cannot
    /// await (e.g., in a `Drop` implementation via `spawn`).
    ///
    /// This method consumes `self`, ensuring no further operations can be
    /// performed after abort.
    pub fn abort(self) {
        warn!("aborting mouse IO task");
        self.task_handle.abort();
    }

    // =========================================================================
    // Low-level Communication
    // =========================================================================

    /// Send a command and receive the response.
    ///
    /// The protocol uses single-packet request/response. Each command
    /// receives exactly one 16-byte response packet (17 bytes with report ID).
    #[instrument(skip(self, cmd))]
    async fn send_command(&self, cmd: &[u8; PACKET_LENGTH]) -> Result<Vec<u8>> {
        trace!(?cmd, "sending command");

        let (response_tx, response_rx) = oneshot::channel();

        self.command_tx
            .send(Command {
                packet: *cmd,
                response_tx,
            })
            .await
            .map_err(|_| MouseError::ChannelClosed)?;

        response_rx.await.map_err(|_| MouseError::ChannelClosed)?
    }

    /// Send a status command (used before writes to sync with device).
    #[instrument(skip(self))]
    async fn send_status_sync(&self) -> Result<()> {
        trace!("sending status sync command");
        let cmd = build_status_cmd();
        let _ = self.send_command(&cmd).await?;
        trace!("status sync completed");
        Ok(())
    }

    /// Check response status byte per protocol spec.
    ///
    /// Per protocol spec section 3 and 9, response byte index 2 (with Report ID at byte 0)
    /// indicates status: 0x00 = success, 0x01 = error/not supported.
    ///
    /// Returns Ok(()) if status is success, Err(NotSupported) if status indicates error.
    #[instrument(skip(response))]
    fn check_response_status(response: &[u8]) -> Result<()> {
        if response.len() < 3 {
            return Err(MouseError::InsufficientData {
                need: 3,
                got: response.len(),
            });
        }

        // Status byte is at index 2 (Report ID at 0, Command at 1, Status at 2)
        if response[2] != 0x00 {
            trace!(
                status = response[2],
                "response indicates error/not supported"
            );
            return Err(MouseError::NotSupported);
        }

        Ok(())
    }

    /// Read memory from the mouse.
    ///
    /// Response format (with report ID at byte 0):
    /// - Byte 0: Report ID (0x08)
    /// - Byte 1: Command ID echo (0x08)
    /// - Byte 2: Status (0x00 = success)
    /// - Byte 3: Address high byte
    /// - Byte 4: Address low byte
    /// - Byte 5: Data length
    /// - Bytes 6+: Data payload
    ///
    /// Per protocol spec section 5.8, we verify that bytes 0-4 of the response
    /// match the request before accepting data.
    #[instrument(skip(self))]
    async fn read_memory(&self, offset: u16, length: u8) -> Result<Vec<u8>> {
        debug!(
            offset = format!("0x{:04X}", offset),
            length, "reading memory"
        );
        let cmd = build_memory_read(offset, length);
        let response = self.send_command(&cmd).await?;

        // Response includes report ID at byte 0, so minimum length is 6 + data
        // Byte 0: report ID, Byte 1: cmd, Byte 2: status, Bytes 3-4: addr, Byte 5: len, Byte 6+: data
        if response.len() < 6 + length as usize {
            error!(
                need = 6 + length as usize,
                got = response.len(),
                "insufficient data in memory read response"
            );
            return Err(MouseError::InsufficientData {
                need: 6 + length as usize,
                got: response.len(),
            });
        }

        // Command echo is at byte 1 (after report ID)
        if response[1] != CMD_MEMORY_READ {
            error!(
                expected = CMD_MEMORY_READ,
                got = response[1],
                "unexpected response command"
            );
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_MEMORY_READ,
                got: response[1],
            });
        }

        Self::check_response_status(&response)?;

        // Per protocol spec section 5.8: verify address echo matches request
        // Response bytes 3-4 should echo the address from request bytes 2-3
        let resp_addr_high = response[3];
        let resp_addr_low = response[4];
        let expected_addr_high = (offset >> 8) as u8;
        let expected_addr_low = (offset & 0xFF) as u8;

        if resp_addr_high != expected_addr_high || resp_addr_low != expected_addr_low {
            warn!(
                expected_addr = format!("0x{:02X}{:02X}", expected_addr_high, expected_addr_low),
                got_addr = format!("0x{:02X}{:02X}", resp_addr_high, resp_addr_low),
                "address mismatch in read response"
            );
            // Note: We log a warning but don't fail, as some devices may not echo the address correctly
        }

        let resp_length = response[5];
        if resp_length != length {
            trace!(
                expected_length = length,
                got_length = resp_length,
                "length mismatch in read response (may be normal)"
            );
        }

        // Extract data bytes (starting at byte 6, after header bytes)
        let data = response[6..6 + length as usize].to_vec();
        trace!(?data, "memory read completed");
        Ok(data)
    }

    /// Read a single byte from memory.
    #[instrument(skip(self))]
    async fn read_memory_byte(&self, offset: u16) -> Result<u8> {
        trace!(offset = format!("0x{:04X}", offset), "reading single byte");
        let data = self.read_memory(offset, 1).await?;
        trace!(value = format!("0x{:02X}", data[0]), "byte read completed");
        Ok(data[0])
    }

    /// Write a single byte to memory.
    ///
    /// Follows the observed protocol pattern: send status command first, then write.
    ///
    /// Response format (with report ID at byte 0):
    /// - Byte 0: Report ID (0x08)
    /// - Byte 1: Command ID echo (0x07)
    /// - Byte 2: Status (0x00 = success)
    #[instrument(skip(self))]
    async fn write_memory(&self, offset: u16, value: u8) -> Result<()> {
        debug!(
            offset = format!("0x{:04X}", offset),
            value = format!("0x{:02X}", value),
            "writing memory"
        );

        self.send_status_sync().await?;
        let cmd = build_memory_write(offset, value);
        let response = self.send_command(&cmd).await?;

        // Validate response echoes the command (command is at byte 1 after report ID)
        if response.len() < 2 {
            error!(
                need = 2,
                got = response.len(),
                "insufficient data in write response"
            );
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_MEMORY_WRITE {
            error!(
                expected = CMD_MEMORY_WRITE,
                got = response[1],
                "unexpected response command"
            );
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_MEMORY_WRITE,
                got: response[1],
            });
        }

        // Check response status byte
        Self::check_response_status(&response)?;

        debug!(
            offset = format!("0x{:04X}", offset),
            "memory write completed"
        );
        Ok(())
    }

    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get the current polling rate.
    #[instrument(skip(self))]
    pub async fn get_polling_rate(&self) -> Result<PollingRate> {
        debug!("getting polling rate");
        let byte = self.read_memory_byte(OFFSET_POLLING_RATE).await?;
        match PollingRate::try_from(byte).ok() {
            Some(rate) => {
                debug!(?rate, "polling rate retrieved");
                Ok(rate)
            }
            None => {
                error!(byte, "invalid polling rate value");
                Err(MouseError::InvalidPollingRate(byte))
            }
        }
    }

    /// Get the current lift-off distance.
    #[instrument(skip(self))]
    pub async fn get_lift_off_distance(&self) -> Result<LiftOffDistance> {
        debug!("getting lift-off distance");
        let byte = self.read_memory_byte(OFFSET_LIFT_OFF_DISTANCE).await?;
        match LiftOffDistance::try_from(byte).ok() {
            Some(lod) => {
                debug!(?lod, "lift-off distance retrieved");
                Ok(lod)
            }
            None => {
                error!(byte, "invalid lift-off distance value");
                Err(MouseError::InvalidLiftOffDistance(byte))
            }
        }
    }

    /// Get the current sleep timeout in seconds.
    #[instrument(skip(self))]
    pub async fn get_sleep_timeout(&self) -> Result<u16> {
        debug!("getting sleep timeout");
        let byte = self.read_memory_byte(OFFSET_SLEEP_TIMEOUT).await?;
        let seconds = (byte as u16) * 10;
        debug!(seconds, "sleep timeout retrieved");
        Ok(seconds)
    }

    impl_bool_setting!(
        get_angle_snapping,
        set_angle_snapping,
        OFFSET_ANGLE_SNAPPING,
        "angle snapping"
    );

    impl_bool_setting!(
        get_ripple_control,
        set_ripple_control,
        OFFSET_RIPPLE_CONTROL,
        "ripple control"
    );

    impl_bool_setting!(
        get_high_speed_mode,
        set_high_speed_mode,
        OFFSET_HIGH_SPEED_MODE,
        "high speed mode"
    );

    /// Get long distance mode state.
    ///
    /// Response format (with report ID at byte 0):
    /// - Byte 0: Report ID (0x08)
    /// - Byte 1: Command ID (0x17)
    /// - Byte 2: Status (0x00 = success, 0x01 = not supported)
    /// - Byte 6: Long range status (0x01 = enabled, 0x00 = disabled)
    ///
    /// Returns `Err(NotSupported)` if the device doesn't support long range mode.
    #[instrument(skip(self))]
    pub async fn get_long_distance_mode(&self) -> Result<bool> {
        debug!("getting long distance mode state");
        let cmd = build_get_long_range_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 7 {
            error!(
                need = 7,
                got = response.len(),
                "insufficient data in wireless status response"
            );
            return Err(MouseError::InsufficientData {
                need: 7,
                got: response.len(),
            });
        }

        // Per protocol spec section 5.15: if byte 2 == 0x01, the device
        // does not support long-range mode
        if response[2] == 0x01 {
            debug!("long distance mode not supported by this device");
            return Err(MouseError::NotSupported);
        }

        // response[6] contains the actual mode status
        let enabled = response[6] == 0x01;
        debug!(enabled, "long distance mode state retrieved");
        Ok(enabled)
    }

    /// Get the full mouse configuration.
    #[instrument(skip(self))]
    pub async fn get_config(&self) -> Result<MouseConfig> {
        info!("retrieving full mouse configuration");
        let config = MouseConfig {
            polling_rate: self.get_polling_rate().await?,
            lift_off_distance: self.get_lift_off_distance().await?,
            sleep_timeout_seconds: self.get_sleep_timeout().await?,
            angle_snapping: self.get_angle_snapping().await?,
            ripple_control: self.get_ripple_control().await?,
            high_speed_mode: self.get_high_speed_mode().await?,
            long_distance_mode: self.get_long_distance_mode().await?,
            debounce_time: self.get_debounce_time().await?,
            motion_sync: self.get_motion_sync().await?,
            moving_off_light_time: self.get_moving_off_light_time().await?,
            performance_time: self.get_performance_time().await?,
            sensor_mode: self.get_sensor_mode().await?,
            sensor_20k: self.get_sensor_20k_mode().await?,
        };
        info!("mouse configuration retrieved successfully");
        Ok(config)
    }

    /// Get battery status.
    ///
    /// Response format (with report ID at byte 0):
    /// - Byte 0: Report ID (0x08)
    /// - Byte 1: Command ID (0x04)
    /// - Byte 2: Status (0x00 = success)
    /// - Byte 6: Battery level (0-100 percentage)
    /// - Byte 7: Charging status (0x01 = charging, 0x00 = not charging)
    /// - Byte 8: Voltage high byte
    /// - Byte 9: Voltage low byte
    #[instrument(skip(self))]
    pub async fn get_battery(&self) -> Result<BatteryStatus> {
        debug!("getting battery status");
        let cmd = build_battery_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 10 {
            error!(
                need = 10,
                got = response.len(),
                "insufficient data in battery response"
            );
            return Err(MouseError::InsufficientData {
                need: 10,
                got: response.len(),
            });
        }

        if response[1] != CMD_BATTERY {
            error!(
                expected = CMD_BATTERY,
                got = response[1],
                "unexpected response command for battery"
            );
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_BATTERY,
                got: response[1],
            });
        }

        Self::check_response_status(&response)?;

        let charging = response[7] == 0x01;
        let voltage_mv = u16::from_be_bytes([response[8], response[9]]);
        let percentage = crate::protocol::voltage_to_percentage_table(voltage_mv);

        debug!(voltage_mv, percentage, charging, "battery status retrieved");
        Ok(BatteryStatus {
            voltage_mv,
            percentage,
            charging,
        })
    }

    /// Get firmware version information.
    #[instrument(skip(self))]
    pub async fn get_firmware_info(&self) -> Result<FirmwareInfo> {
        debug!("getting firmware information");

        let mouse_cmd = build_mouse_firmware_cmd();
        let mouse_response = self.send_command(&mouse_cmd).await?;

        // Response format (with report ID at byte 0):
        // - Byte 0: Report ID (0x08)
        // - Byte 1: Command ID (0x12)
        // - Byte 6: Major version (decimal)
        // - Byte 7: Minor version (hex format, display as-is)
        let mouse_version = if mouse_response.len() >= 8
            && mouse_response[0] == REPORT_ID
            && mouse_response[1] == CMD_MOUSE_FIRMWARE
        {
            let major = mouse_response[6];
            let minor = mouse_response[7];
            // Per protocol: format as "v{major}.{minor:02x}"
            let version = format_firmware_version(major, minor);
            trace!(mouse_version = %version, "mouse firmware version parsed");
            version
        } else {
            trace!("could not parse mouse firmware version");
            "Unknown".to_string()
        };

        // Receiver firmware (only meaningful in wireless mode)
        let receiver_cmd = build_receiver_firmware_cmd();
        let receiver_response = self.send_command(&receiver_cmd).await?;

        let receiver_version = match self.mode {
            ConnectionMode::Wireless => {
                if receiver_response.len() >= 8
                    && receiver_response[0] == REPORT_ID
                    && receiver_response[1] == CMD_RECEIVER_FIRMWARE
                {
                    let major = receiver_response[6];
                    let minor = receiver_response[7];
                    // Per protocol: format as "v{major}.{minor:02x}"
                    let version = format_firmware_version(major, minor);
                    trace!(receiver_version = %version, "receiver firmware version parsed");
                    Some(version)
                } else {
                    trace!("could not parse receiver firmware version");
                    None
                }
            }
            ConnectionMode::Wired => {
                trace!("skipping receiver firmware (wired mode)");
                None
            }
        };

        debug!(
            mouse_version = %mouse_version,
            ?receiver_version,
            "firmware information retrieved"
        );
        Ok(FirmwareInfo {
            mouse_version,
            receiver_version,
        })
    }

    /// Get the current DPI count (number of active DPI stages).
    #[instrument(skip(self))]
    pub async fn get_dpi_count(&self) -> Result<u8> {
        debug!("getting DPI count");
        let byte = self.read_memory_byte(OFFSET_MAX_DPI).await?;
        debug!(dpi_count = byte, "DPI count retrieved");
        Ok(byte)
    }

    /// Get the current DPI index (0-7).
    #[instrument(skip(self))]
    pub async fn get_current_dpi_index(&self) -> Result<u8> {
        debug!("getting current DPI index");
        let byte = self.read_memory_byte(OFFSET_CURRENT_DPI).await?;
        debug!(dpi_index = byte, "current DPI index retrieved");
        Ok(byte)
    }

    /// Get a specific DPI stage value.
    #[instrument(skip(self))]
    pub async fn get_dpi_value(&self, stage: u8) -> Result<u16> {
        if stage >= 8 {
            return Err(MouseError::InvalidDpiStage(stage));
        }
        debug!(stage, "getting DPI value");
        let address = OFFSET_DPI_VALUES + (stage as u16 * 4);
        let data = self.read_memory(address, 4).await?;
        let dpi = decode_dpi(&[data[0], data[1], data[2], data[3]]);
        debug!(stage, dpi, "DPI value retrieved");
        Ok(dpi)
    }

    /// Get a specific DPI stage color.
    #[instrument(skip(self))]
    pub async fn get_dpi_color(&self, stage: u8) -> Result<[u8; 3]> {
        if stage >= 8 {
            return Err(MouseError::InvalidDpiStage(stage));
        }
        debug!(stage, "getting DPI color");
        let address = OFFSET_DPI_COLORS + (stage as u16 * 4);
        let data = self.read_memory(address, 4).await?;
        let color = [data[0], data[1], data[2]];
        debug!(stage, ?color, "DPI color retrieved");
        Ok(color)
    }

    /// Get all DPI stages.
    #[instrument(skip(self))]
    pub async fn get_dpi_stages(&self) -> Result<Vec<DpiStage>> {
        debug!("getting all DPI stages");
        let count = self.get_dpi_count().await?;
        let mut stages = Vec::with_capacity(count as usize);

        for i in 0..count {
            let value = self.get_dpi_value(i).await?;
            let color = self.get_dpi_color(i).await?;
            stages.push(DpiStage { value, color });
        }

        debug!(count = stages.len(), "all DPI stages retrieved");
        Ok(stages)
    }

    /// Get debounce time in milliseconds.
    #[instrument(skip(self))]
    pub async fn get_debounce_time(&self) -> Result<u8> {
        debug!("getting debounce time");
        let byte = self.read_memory_byte(OFFSET_DEBOUNCE_TIME).await?;
        debug!(debounce_ms = byte, "debounce time retrieved");
        Ok(byte)
    }

    impl_bool_setting!(
        get_motion_sync,
        set_motion_sync,
        OFFSET_MOTION_SYNC,
        "motion sync"
    );

    impl_bool_setting!(
        get_sensor_20k_mode,
        set_sensor_20k_mode,
        OFFSET_SENSOR_20K,
        "20K sensor mode"
    );

    /// Get sensor mode (low power vs high performance).
    #[instrument(skip(self))]
    pub async fn get_sensor_mode(&self) -> Result<SensorMode> {
        debug!("getting sensor mode");
        let byte = self.read_memory_byte(OFFSET_SENSOR_MODE).await?;
        match SensorMode::try_from(byte).ok() {
            Some(mode) => {
                debug!(?mode, "sensor mode retrieved");
                Ok(mode)
            }
            None => {
                // Default to low power if unknown
                warn!(byte, "unknown sensor mode value, defaulting to LowPower");
                Ok(SensorMode::LowPower)
            }
        }
    }

    /// Get light settings.
    #[instrument(skip(self))]
    pub async fn get_light_settings(&self) -> Result<LightSettings> {
        debug!("getting light settings");
        let data = self.read_memory(OFFSET_LIGHT_SETTINGS, 7).await?;
        let state_byte = self.read_memory_byte(OFFSET_LIGHT_STATE).await?;

        let mode = LightMode::try_from(data[0]).ok().unwrap_or(LightMode::Off);
        let color = [data[1], data[2], data[3]];
        let speed = data[4];
        let brightness = data[5];
        let enabled = state_byte == 0x01;

        let settings = LightSettings {
            mode,
            color,
            speed,
            brightness,
            enabled,
        };
        debug!(?settings, "light settings retrieved");
        Ok(settings)
    }

    /// Get DPI effect settings.
    #[instrument(skip(self))]
    pub async fn get_dpi_effect_settings(&self) -> Result<DpiEffectSettings> {
        debug!("getting DPI effect settings");

        let mode_byte = self.read_memory_byte(OFFSET_DPI_EFFECT_MODE).await?;
        let brightness_byte = self.read_memory_byte(OFFSET_DPI_EFFECT_BRIGHTNESS).await?;
        let speed_byte = self.read_memory_byte(OFFSET_DPI_EFFECT_SPEED).await?;
        let state_byte = self.read_memory_byte(OFFSET_DPI_EFFECT_STATE).await?;

        let mode = DpiEffectMode::try_from(mode_byte)
            .ok()
            .unwrap_or(DpiEffectMode::Off);
        let brightness = decode_brightness(brightness_byte);
        let speed = speed_byte.clamp(1, 10);
        let enabled = state_byte == 0x01;

        let settings = DpiEffectSettings {
            mode,
            brightness,
            speed,
            enabled,
        };
        debug!(?settings, "DPI effect settings retrieved");
        Ok(settings)
    }

    /// Get a specific key function (0-7).
    #[instrument(skip(self))]
    pub async fn get_key_function(&self, key_index: u8) -> Result<KeyFunction> {
        if key_index >= KEY_FUNCTION_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(key_index)); // Reuse error for invalid index
        }
        debug!(key_index, "getting key function");
        let address = OFFSET_KEY_FUNCTIONS + (key_index as u16 * 4);
        let data = self.read_memory(address, 4).await?;
        let bytes = [data[0], data[1], data[2], data[3]];

        let function = KeyFunction::decode(&bytes).unwrap_or_default();
        debug!(key_index, ?function, "key function retrieved");
        Ok(function)
    }

    /// Get all key functions.
    #[instrument(skip(self))]
    pub async fn get_all_key_functions(&self) -> Result<[KeyFunction; KEY_FUNCTION_COUNT]> {
        debug!("getting all key functions");
        let mut functions = [KeyFunction::default(); KEY_FUNCTION_COUNT];

        for i in 0..KEY_FUNCTION_COUNT {
            functions[i] = self.get_key_function(i as u8).await?;
        }

        debug!("all key functions retrieved");
        Ok(functions)
    }

    /// Get moving off light time.
    #[instrument(skip(self))]
    pub async fn get_moving_off_light_time(&self) -> Result<u8> {
        debug!("getting moving off light time");
        let byte = self.read_memory_byte(OFFSET_MOVING_OFF_LIGHT).await?;
        debug!(time = byte, "moving off light time retrieved");
        Ok(byte)
    }

    /// Get performance/sleep time value.
    #[instrument(skip(self))]
    pub async fn get_performance_time(&self) -> Result<SleepTime> {
        debug!("getting performance time");
        let byte = self.read_memory_byte(OFFSET_PERFORMANCE_TIME).await?;
        match SleepTime::try_from(byte).ok() {
            Some(time) => {
                debug!(?time, "performance time retrieved");
                Ok(time)
            }
            None => {
                warn!(byte, "unknown performance time value, defaulting to Sec10");
                Ok(SleepTime::Sec10)
            }
        }
    }

    /// Get a shortcut key definition (0-7).
    #[instrument(skip(self))]
    pub async fn get_shortcut_key(&self, slot: u8) -> Result<ShortcutKey> {
        if slot >= SHORTCUT_KEY_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(slot));
        }
        debug!(slot, "getting shortcut key");

        let address = OFFSET_SHORTCUT_KEYS + (slot as u16 * SHORTCUT_KEY_SLOT_SIZE as u16);
        let mut data = [0u8; 32];

        // Read in chunks of 10 bytes (max per read command)
        let chunk1 = self.read_memory(address, 10).await?;
        let chunk2 = self.read_memory(address + 10, 10).await?;
        let chunk3 = self.read_memory(address + 20, 10).await?;
        let chunk4 = self.read_memory(address + 30, 2).await?;

        data[0..10].copy_from_slice(&chunk1);
        data[10..20].copy_from_slice(&chunk2);
        data[20..30].copy_from_slice(&chunk3);
        data[30..32].copy_from_slice(&chunk4);

        let shortcut = ShortcutKey::decode(&data).unwrap_or_default();
        debug!(
            slot,
            events = shortcut.events.len(),
            "shortcut key retrieved"
        );
        Ok(shortcut)
    }

    /// Get a macro definition (0-7).
    #[instrument(skip(self))]
    pub async fn get_macro(&self, slot: u8) -> Result<Macro> {
        if slot >= MACRO_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(slot));
        }
        debug!(slot, "getting macro");

        let base_address = OFFSET_MACROS + (slot as u16 * MACRO_SLOT_SIZE as u16);

        let mut data = [0u8; Macro::SLOT_SIZE];
        let mut offset = 0u16;
        while offset < Macro::SLOT_SIZE as u16 {
            let remaining = Macro::SLOT_SIZE as u16 - offset;
            let chunk_len = remaining.min(10) as u8;
            let chunk = self.read_memory(base_address + offset, chunk_len).await?;
            data[offset as usize..offset as usize + chunk.len()].copy_from_slice(&chunk);
            offset += chunk_len as u16;
        }

        let macro_def = Macro::decode(&data)
            .ok_or_else(|| MouseError::InsufficientData { need: 32, got: 0 })?;

        debug!(slot, name = %macro_def.name, events = macro_def.events.len(), "macro retrieved");
        Ok(macro_def)
    }

    /// Read the entire flash memory (256 bytes of basic configuration).
    ///
    /// This is useful for debugging or making a full backup of the configuration.
    /// Per protocol spec section 8.2, reads are done in 10-byte chunks.
    #[instrument(skip(self))]
    pub async fn read_full_flash(&self) -> Result<[u8; 256]> {
        debug!("reading full flash memory");
        let mut flash = [0u8; 256];
        let mut address = 0u16;

        while address < 256 {
            let len = (256 - address).min(10) as u8;
            let data = self.read_memory(address, len).await?;
            flash[address as usize..address as usize + len as usize].copy_from_slice(&data);
            address += len as u16;
        }

        debug!("full flash memory read completed");
        Ok(flash)
    }

    /// Get the current profile index (0-3).
    #[instrument(skip(self))]
    pub async fn get_current_profile(&self) -> Result<u8> {
        debug!("getting current profile");
        let cmd = build_get_current_config_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 7 {
            error!(
                need = 7,
                got = response.len(),
                "insufficient data in get profile response"
            );
            return Err(MouseError::InsufficientData {
                need: 7,
                got: response.len(),
            });
        }

        if response[1] != CMD_GET_CURRENT_CONFIG {
            error!(
                expected = CMD_GET_CURRENT_CONFIG,
                got = response[1],
                "unexpected response command for get profile"
            );
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_GET_CURRENT_CONFIG,
                got: response[1],
            });
        }

        let profile_index = response[6];
        debug!(profile_index, "current profile retrieved");
        Ok(profile_index)
    }

    /// Check if mouse is online (connected to dongle, for wireless).
    #[instrument(skip(self))]
    pub async fn check_online(&self) -> Result<bool> {
        debug!("checking if mouse is online");
        let cmd = build_status_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 7 {
            error!(
                need = 7,
                got = response.len(),
                "insufficient data in online check response"
            );
            return Err(MouseError::InsufficientData {
                need: 7,
                got: response.len(),
            });
        }

        let online = response[6] == 0x01;
        debug!(online, "online status retrieved");
        Ok(online)
    }

    /// Get device address (for wireless pairing identification).
    #[instrument(skip(self))]
    pub async fn get_device_address(&self) -> Result<[u8; 3]> {
        debug!("getting device address");
        let cmd = build_status_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 10 {
            error!(
                need = 10,
                got = response.len(),
                "insufficient data in device address response"
            );
            return Err(MouseError::InsufficientData {
                need: 10,
                got: response.len(),
            });
        }

        // Per protocol section 5.3 (with Report ID at byte 0):
        // Byte 6: Online status
        // Byte 7: Device address byte 2
        // Byte 8: Device address byte 1
        // Byte 9: Device address byte 0
        // The address is stored in big-endian order (byte 2, byte 1, byte 0)
        let address = [response[9], response[8], response[7]];
        debug!(?address, "device address retrieved");
        Ok(address)
    }

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Set the polling rate.
    #[instrument(skip(self))]
    pub async fn set_polling_rate(&self, rate: PollingRate) -> Result<()> {
        info!(?rate, "setting polling rate");
        self.write_memory(OFFSET_POLLING_RATE, rate.into()).await?;
        info!(?rate, "polling rate set successfully");
        Ok(())
    }

    /// Set the lift-off distance.
    #[instrument(skip(self))]
    pub async fn set_lift_off_distance(&self, lod: LiftOffDistance) -> Result<()> {
        info!(?lod, "setting lift-off distance");
        self.write_memory(OFFSET_LIFT_OFF_DISTANCE, lod.into())
            .await?;
        info!(?lod, "lift-off distance set successfully");
        Ok(())
    }

    /// Set the sleep timeout in seconds.
    ///
    /// Must be a multiple of 10. Maximum value is 2550 seconds.
    /// The value is written to both the primary and secondary memory locations.
    ///
    /// Returns the actual sleep timeout that was applied (rounded down to a multiple of 10).
    #[instrument(skip(self))]
    pub async fn set_sleep_timeout(&self, seconds: u16) -> Result<u16> {
        info!(seconds, "setting sleep timeout");
        if seconds != 10 {
            warn!("firmware bug: sleep timeout is always 10s regardless of configured value");
        }

        if seconds > MAX_SLEEP_TIMEOUT_SECONDS {
            error!(
                seconds,
                max = MAX_SLEEP_TIMEOUT_SECONDS,
                "sleep timeout exceeds maximum"
            );
            return Err(MouseError::InvalidSleepTimeout(seconds));
        }

        let value = (seconds / 10) as u8;
        let actual_seconds = value as u16 * 10;
        trace!(
            raw_value = value,
            actual_seconds, "calculated raw timeout value"
        );

        if actual_seconds != seconds {
            warn!(
                requested = seconds,
                actual = actual_seconds,
                "sleep timeout rounded down to multiple of 10"
            );
        }

        self.write_memory(OFFSET_SLEEP_TIMEOUT_SECONDARY, value)
            .await?;
        self.write_memory(OFFSET_SLEEP_TIMEOUT, value).await?;

        info!(actual_seconds, "sleep timeout set successfully");
        Ok(actual_seconds)
    }

    /// Set long distance mode state.
    ///
    /// This uses a special command (0x16) instead of memory write.
    #[instrument(skip(self))]
    pub async fn set_long_distance_mode(&self, enabled: bool) -> Result<()> {
        info!(enabled, "setting long distance mode");

        // Send status command first (observed in all write sequences)
        self.send_status_sync().await?;

        let cmd = build_set_long_range_cmd(enabled);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            error!(
                need = 2,
                got = response.len(),
                "insufficient data in long distance mode response"
            );
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_LONG_DISTANCE {
            error!(
                expected = CMD_LONG_DISTANCE,
                got = response[1],
                "unexpected response command for long distance mode"
            );
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_LONG_DISTANCE,
                got: response[1],
            });
        }

        info!(enabled, "long distance mode set successfully");
        Ok(())
    }

    /// Apply a full configuration.
    #[instrument(skip_all)]
    pub async fn set_config(&self, config: &MouseConfig) -> Result<()> {
        info!("applying full mouse configuration");
        debug!(?config, "configuration to apply");

        self.set_polling_rate(config.polling_rate).await?;
        self.set_lift_off_distance(config.lift_off_distance).await?;
        self.set_sleep_timeout(config.sleep_timeout_seconds).await?;
        self.set_angle_snapping(config.angle_snapping).await?;
        self.set_ripple_control(config.ripple_control).await?;
        self.set_high_speed_mode(config.high_speed_mode).await?;
        self.set_long_distance_mode(config.long_distance_mode)
            .await?;
        self.set_debounce_time(config.debounce_time).await?;
        self.set_motion_sync(config.motion_sync).await?;
        self.set_moving_off_light_time(config.moving_off_light_time)
            .await?;
        self.set_performance_time(config.performance_time).await?;
        self.set_sensor_mode(config.sensor_mode).await?;
        self.set_sensor_20k_mode(config.sensor_20k).await?;

        info!("mouse configuration applied successfully");
        Ok(())
    }

    /// Set the DPI count (number of active DPI stages, 1-8).
    #[instrument(skip(self))]
    pub async fn set_dpi_count(&self, count: u8) -> Result<()> {
        if count == 0 || count > 8 {
            return Err(MouseError::InvalidDpiStage(count));
        }
        info!(count, "setting DPI count");
        self.write_memory(OFFSET_MAX_DPI, count).await?;
        info!(count, "DPI count set successfully");
        Ok(())
    }

    /// Set the current DPI index (0-7).
    #[instrument(skip(self))]
    pub async fn set_current_dpi_index(&self, index: u8) -> Result<()> {
        if index >= 8 {
            return Err(MouseError::InvalidDpiStage(index));
        }
        info!(index, "setting current DPI index");
        self.write_memory(OFFSET_CURRENT_DPI, index).await?;
        info!(index, "current DPI index set successfully");
        Ok(())
    }

    /// Set a specific DPI stage value.
    #[instrument(skip(self))]
    pub async fn set_dpi_value(&self, stage: u8, dpi: u16) -> Result<()> {
        if stage >= 8 {
            return Err(MouseError::InvalidDpiStage(stage));
        }
        if dpi < DPI_MIN || dpi > DPI_MAX {
            return Err(MouseError::InvalidDpiValue(dpi));
        }
        info!(stage, dpi, "setting DPI value");

        let address = OFFSET_DPI_VALUES + (stage as u16 * 4);
        let encoded = encode_dpi(dpi);

        self.send_status_sync().await?;
        let cmd = build_flash_write(address, &encoded);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_WRITE_FLASH {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_WRITE_FLASH,
                got: response[1],
            });
        }

        info!(stage, dpi, "DPI value set successfully");
        Ok(())
    }

    /// Set a specific DPI stage color.
    #[instrument(skip(self))]
    pub async fn set_dpi_color(&self, stage: u8, color: [u8; 3]) -> Result<()> {
        if stage >= 8 {
            return Err(MouseError::InvalidDpiStage(stage));
        }
        info!(stage, ?color, "setting DPI color");

        let address = OFFSET_DPI_COLORS + (stage as u16 * 4);
        let data = [color[0], color[1], color[2], 0x00];

        self.send_status_sync().await?;
        let cmd = build_flash_write(address, &data);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_WRITE_FLASH {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_WRITE_FLASH,
                got: response[1],
            });
        }

        info!(stage, ?color, "DPI color set successfully");
        Ok(())
    }

    /// Set debounce time in milliseconds (0-30).
    #[instrument(skip(self))]
    pub async fn set_debounce_time(&self, ms: u8) -> Result<()> {
        if ms > 30 {
            return Err(MouseError::InvalidDebounceTime(ms));
        }
        info!(ms, "setting debounce time");
        self.write_memory(OFFSET_DEBOUNCE_TIME, ms).await?;
        info!(ms, "debounce time set successfully");
        Ok(())
    }

    /// Set sensor mode (low power vs high performance).
    #[instrument(skip(self))]
    pub async fn set_sensor_mode(&self, mode: SensorMode) -> Result<()> {
        info!(?mode, "setting sensor mode");
        self.write_memory(OFFSET_SENSOR_MODE, mode.into()).await?;
        info!(?mode, "sensor mode set successfully");
        Ok(())
    }

    /// Set light settings.
    #[instrument(skip(self))]
    pub async fn set_light_settings(&self, settings: &LightSettings) -> Result<()> {
        info!(?settings, "setting light settings");

        self.send_status_sync().await?;
        let data = [
            settings.mode.into(),
            settings.color[0],
            settings.color[1],
            settings.color[2],
            settings.speed,
            settings.brightness,
            0x00, // Reserved
        ];

        let cmd = build_flash_write(OFFSET_LIGHT_SETTINGS, &data);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_WRITE_FLASH {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_WRITE_FLASH,
                got: response[1],
            });
        }

        self.write_memory(
            OFFSET_LIGHT_STATE,
            if settings.enabled { 0x01 } else { 0x00 },
        )
        .await?;

        info!("light settings set successfully");
        Ok(())
    }

    /// Set DPI effect settings.
    #[instrument(skip(self))]
    pub async fn set_dpi_effect_settings(&self, settings: &DpiEffectSettings) -> Result<()> {
        info!(?settings, "setting DPI effect settings");

        self.write_memory(OFFSET_DPI_EFFECT_MODE, settings.mode.into())
            .await?;
        self.write_memory(
            OFFSET_DPI_EFFECT_BRIGHTNESS,
            encode_brightness(settings.brightness),
        )
        .await?;
        self.write_memory(OFFSET_DPI_EFFECT_SPEED, settings.speed.clamp(1, 10))
            .await?;
        self.write_memory(
            OFFSET_DPI_EFFECT_STATE,
            if settings.enabled { 0x01 } else { 0x00 },
        )
        .await?;

        info!("DPI effect settings set successfully");
        Ok(())
    }

    /// Set a specific key function (0-7).
    #[instrument(skip(self))]
    pub async fn set_key_function(&self, key_index: u8, function: &KeyFunction) -> Result<()> {
        if key_index >= KEY_FUNCTION_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(key_index));
        }
        info!(key_index, ?function, "setting key function");

        let address = OFFSET_KEY_FUNCTIONS + (key_index as u16 * 4);
        let encoded = function.encode();

        self.send_status_sync().await?;
        let cmd = build_flash_write(address, &encoded);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_WRITE_FLASH {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_WRITE_FLASH,
                got: response[1],
            });
        }

        info!(key_index, "key function set successfully");
        Ok(())
    }

    /// Set moving off light time.
    #[instrument(skip(self))]
    pub async fn set_moving_off_light_time(&self, time: u8) -> Result<()> {
        info!(time, "setting moving off light time");
        self.write_memory(OFFSET_MOVING_OFF_LIGHT, time).await?;
        info!(time, "moving off light time set successfully");
        Ok(())
    }

    /// Set performance/sleep time value.
    #[instrument(skip(self))]
    pub async fn set_performance_time(&self, time: SleepTime) -> Result<()> {
        info!(?time, "setting performance time");
        self.write_memory(OFFSET_PERFORMANCE_TIME, time.into())
            .await?;
        info!(?time, "performance time set successfully");
        Ok(())
    }

    /// Set a shortcut key definition (0-7).
    #[instrument(skip(self))]
    pub async fn set_shortcut_key(&self, slot: u8, shortcut: &ShortcutKey) -> Result<()> {
        if slot >= SHORTCUT_KEY_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(slot));
        }
        info!(slot, events = shortcut.events.len(), "setting shortcut key");

        let address = OFFSET_SHORTCUT_KEYS + (slot as u16 * SHORTCUT_KEY_SLOT_SIZE as u16);
        let encoded = shortcut.encode();

        self.send_status_sync().await?;
        let cmd1 = build_flash_write(address, &encoded[0..10]);
        self.send_command(&cmd1).await?;

        let cmd2 = build_flash_write(address + 10, &encoded[10..20]);
        self.send_command(&cmd2).await?;

        let cmd3 = build_flash_write(address + 20, &encoded[20..30]);
        self.send_command(&cmd3).await?;

        let cmd4 = build_flash_write(address + 30, &encoded[30..32]);
        self.send_command(&cmd4).await?;

        info!(slot, "shortcut key set successfully");
        Ok(())
    }

    /// Set a macro definition (0-7).
    #[instrument(skip(self))]
    pub async fn set_macro(&self, slot: u8, macro_def: &Macro) -> Result<()> {
        if slot >= MACRO_COUNT as u8 {
            return Err(MouseError::InvalidDpiStage(slot));
        }
        info!(slot, name = %macro_def.name, events = macro_def.events.len(), "setting macro");

        let base_address = OFFSET_MACROS + (slot as u16 * MACRO_SLOT_SIZE as u16);

        let encoded = macro_def.encode();
        self.send_status_sync().await?;

        // Write in chunks of 10 bytes (max per flash write command)
        // Macro slot is 384 bytes, so we need 39 writes (384 / 10 = 38.4)
        let mut offset = 0u16;
        while offset < Macro::SLOT_SIZE as u16 {
            let remaining = Macro::SLOT_SIZE as u16 - offset;
            let chunk_len = remaining.min(10) as usize;
            let chunk = &encoded[offset as usize..offset as usize + chunk_len];

            let cmd = build_flash_write(base_address + offset, chunk);
            self.send_command(&cmd).await?;

            offset += chunk_len as u16;
        }

        info!(slot, "macro set successfully");
        Ok(())
    }

    /// Set the current profile (0-3).
    #[instrument(skip(self))]
    pub async fn set_current_profile(&self, profile_index: u8) -> Result<()> {
        if profile_index > 3 {
            return Err(MouseError::InvalidProfile(profile_index));
        }
        info!(profile_index, "setting current profile");

        self.send_status_sync().await?;
        let cmd = build_set_current_config_cmd(profile_index);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_SET_CURRENT_CONFIG {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_SET_CURRENT_CONFIG,
                got: response[1],
            });
        }

        info!(profile_index, "current profile set successfully");
        Ok(())
    }

    /// Factory reset the mouse.
    ///
    /// Warning: This will reset all settings to factory defaults.
    /// After calling this, wait up to 1200ms for the device to complete the reset.
    #[instrument(skip(self))]
    pub async fn factory_reset(&self) -> Result<()> {
        info!("performing factory reset");
        self.send_status_sync().await?;
        let cmd = build_clear_setting_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_CLEAR_SETTING {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_CLEAR_SETTING,
                got: response[1],
            });
        }

        info!("factory reset command sent successfully");
        Ok(())
    }

    /// Enter pairing mode (wireless dongle only).
    ///
    /// The dongle will enter pairing mode for 62 seconds.
    #[instrument(skip(self))]
    pub async fn enter_pairing_mode(&self) -> Result<()> {
        info!("entering pairing mode");
        self.send_status_sync().await?;
        let cmd = build_dongle_enter_pair_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_DONGLE_ENTER_PAIR {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_DONGLE_ENTER_PAIR,
                got: response[1],
            });
        }

        info!("pairing mode entered successfully");
        Ok(())
    }

    /// Get the current pairing status.
    #[instrument(skip(self))]
    pub async fn get_pair_state(&self) -> Result<(PairStatus, u8)> {
        debug!("getting pairing state");
        let cmd = build_get_pair_state_cmd();
        let response = self.send_command(&cmd).await?;

        if response.len() < 8 {
            return Err(MouseError::InsufficientData {
                need: 8,
                got: response.len(),
            });
        }

        if response[1] != CMD_GET_PAIR_STATE {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_GET_PAIR_STATE,
                got: response[1],
            });
        }

        let status = PairStatus::try_from(response[6])
            .ok()
            .unwrap_or(PairStatus::Idle);
        let time_remaining = response[7];

        debug!(?status, time_remaining, "pairing state retrieved");
        Ok((status, time_remaining))
    }

    /// Notify the device of driver connection status.
    #[instrument(skip(self))]
    pub async fn set_driver_status(&self, connected: bool) -> Result<()> {
        info!(connected, "setting driver status");

        let cmd = build_pc_driver_status_cmd(connected);
        let response = self.send_command(&cmd).await?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_PC_DRIVER_STATUS {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_PC_DRIVER_STATUS,
                got: response[1],
            });
        }

        info!(connected, "driver status set successfully");
        Ok(())
    }

    /// Perform device handshake and get device information.
    #[instrument(skip(self))]
    pub async fn handshake(&self) -> Result<DeviceInfo> {
        debug!("performing handshake");

        // Generate random bytes for handshake token
        let random_bytes: [u8; 4] = [
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                & 0xFF) as u8,
            ((std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                >> 8)
                & 0xFF) as u8,
            ((std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                >> 16)
                & 0xFF) as u8,
            ((std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                >> 24)
                & 0xFF) as u8,
        ];

        let cmd = build_handshake_cmd(&random_bytes);
        let response = self.send_command(&cmd).await?;

        if response.len() < 12 {
            return Err(MouseError::InsufficientData {
                need: 12,
                got: response.len(),
            });
        }

        if response[1] != CMD_ENCRYPTION_DATA {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_ENCRYPTION_DATA,
                got: response[1],
            });
        }

        // Check response status byte
        Self::check_response_status(&response)?;

        // Per protocol spec section 5.1 (with Report ID at byte 0):
        // Byte 0: Report ID (0x08)
        // Byte 1: Command ID echo (0x01)
        // Byte 2: Status (0x00 = success)
        // Bytes 6-9: Echo of random bytes sent
        // Byte 10: CID (Company ID)
        // Byte 11: MID (Model ID)
        // Byte 12: Type (connection type)
        let cid = response[10];
        let mid = response[11];
        let conn_type_byte = response[12];
        let connection_type = ConnectionType::try_from(conn_type_byte)
            .ok()
            .unwrap_or(ConnectionType::WirelessStandard);

        let info = DeviceInfo {
            cid,
            mid,
            connection_type,
            online: true,
            address: [0; 3], // Will be filled by check_online if needed
        };

        debug!(?info, "handshake completed");
        Ok(info)
    }
}
