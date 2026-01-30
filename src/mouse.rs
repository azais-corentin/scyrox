//! Mouse communication and configuration API.

use std::time::Duration;

use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient};
use nusb::{Endpoint, Interface, MaybeFuture};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::error::{MouseError, Result};
use crate::protocol::*;
use crate::types::*;

/// Maximum sleep timeout in seconds (0xFF * 10).
pub const MAX_SLEEP_TIMEOUT_SECONDS: u16 = 2550;

/// Mouse device handle for communication.
pub struct Mouse {
    interface: Interface,
    endpoint: Endpoint<Interrupt, In>,
    mode: ConnectionMode,
}

impl Mouse {
    /// Open a connection to the mouse.
    ///
    /// Searches for the mouse by vendor/product ID and claims the configuration interface.
    #[instrument]
    pub fn open() -> Result<Self> {
        debug!("searching for mouse device");

        // Find the mouse device
        let (device_info, mode) = PRODUCT_IDS
            .iter()
            .find_map(|&pid| {
                let device = nusb::list_devices()
                    .wait()
                    .ok()?
                    .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == pid)?;
                let mode = match pid {
                    PID_WIRED => ConnectionMode::Wired,
                    PID_WIRELESS => ConnectionMode::Wireless,
                    _ => ConnectionMode::Wireless,
                };
                Some((device, mode))
            })
            .ok_or_else(|| {
                error!(vid = VENDOR_ID, ?PRODUCT_IDS, "mouse device not found");
                MouseError::NotFound {
                    vid: VENDOR_ID,
                    pids: PRODUCT_IDS.to_vec(),
                }
            })?;

        debug!(?mode, "found mouse device");

        let device = device_info.open().wait()?;

        // Detach kernel driver and claim interface
        let interface = device.detach_and_claim_interface(INTERFACE_NUM).wait()?;

        // Create interrupt endpoint for responses
        let endpoint = interface.endpoint::<Interrupt, In>(INTERRUPT_EP_IN)?;

        info!(?mode, "mouse connection established");

        Ok(Mouse {
            interface,
            endpoint,
            mode,
        })
    }

    /// Get the connection mode (wired or wireless).
    pub fn connection_mode(&self) -> ConnectionMode {
        trace!(mode = ?self.mode, "returning connection mode");
        self.mode
    }

    // =========================================================================
    // Low-level Communication
    // =========================================================================

    /// Send a command and receive the response.
    #[instrument(skip(self, cmd))]
    fn send_command(&mut self, cmd: &[u8; 17]) -> Result<Vec<u8>> {
        trace!(cmd = ?cmd, "sending command");
        let packet_size = self.mode.packet_size();

        // Submit read buffer BEFORE sending command
        let buf = Buffer::new(packet_size);
        self.endpoint.submit(buf);

        // Send command via control transfer
        let result = self
            .interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: 0x09, // SET_REPORT
                    value: 0x0208, // Report Type (Output=2) | Report ID (0x08)
                    index: INTERFACE_NUM as u16,
                    data: cmd,
                },
                Duration::from_millis(100),
            )
            .wait();

        if result.is_err() {
            warn!("control transfer with Class request failed, trying Vendor request");
            // Try alternative: vendor-specific transfer
            self.interface
                .control_out(
                    ControlOut {
                        control_type: ControlType::Vendor,
                        recipient: Recipient::Interface,
                        request: 0x09,
                        value: 0x0208,
                        index: INTERFACE_NUM as u16,
                        data: cmd,
                    },
                    Duration::from_millis(100),
                )
                .wait()?;
        }

        // Wait for response with longer timeout
        let mut response = vec![0u8; packet_size];
        let mut bytes_read = 0;

        for i in 0..3 {
            if i > 0 {
                trace!(attempt = i + 1, "retrying response read");
                let buf = Buffer::new(packet_size);
                self.endpoint.submit(buf);
            }

            match self.endpoint.wait_next_complete(Duration::from_millis(200)) {
                Some(completion) => {
                    if completion.status.is_ok() {
                        let len = completion.buffer.len();
                        if len > 0 {
                            let copy_len = len.min(response.len());
                            response[..copy_len].copy_from_slice(&completion.buffer[..copy_len]);
                            bytes_read = copy_len;
                            trace!(bytes_read, "received response data");
                        }
                    } else {
                        trace!(?completion.status, "completion status not ok");
                        break;
                    }
                }
                None => {
                    if i == 0 {
                        error!("timeout waiting for response on first attempt");
                        return Err(MouseError::Timeout);
                    }
                    trace!(attempt = i + 1, "no more data available");
                    break;
                }
            }
        }

        if bytes_read == 0 {
            error!("no response data received after all attempts");
            return Err(MouseError::Timeout);
        }

        debug!(bytes_read, "command completed successfully");
        Ok(response[..bytes_read].to_vec())
    }

    /// Send a status command (used before writes to sync with device).
    #[instrument(skip(self))]
    fn send_status_sync(&mut self) -> Result<()> {
        trace!("sending status sync command");
        let cmd = build_status_cmd();
        let _ = self.send_command(&cmd)?;
        trace!("status sync completed");
        Ok(())
    }

    /// Read memory from the mouse.
    #[instrument(skip(self))]
    fn read_memory(&mut self, offset: u16, length: u8) -> Result<Vec<u8>> {
        debug!(
            offset = format!("0x{:04X}", offset),
            length, "reading memory"
        );
        let cmd = build_memory_read(offset, length);
        let response = self.send_command(&cmd)?;

        // Validate response
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

        // Extract data bytes (starting at byte 6)
        let data = response[6..6 + length as usize].to_vec();
        trace!(?data, "memory read completed");
        Ok(data)
    }

    /// Read a single byte from memory.
    #[instrument(skip(self))]
    fn read_memory_byte(&mut self, offset: u16) -> Result<u8> {
        trace!(offset = format!("0x{:04X}", offset), "reading single byte");
        let data = self.read_memory(offset, 1)?;
        trace!(value = format!("0x{:02X}", data[0]), "byte read completed");
        Ok(data[0])
    }

    /// Write a single byte to memory.
    ///
    /// Follows the observed protocol pattern: send status command first, then write.
    #[instrument(skip(self))]
    fn write_memory(&mut self, offset: u16, value: u8) -> Result<()> {
        debug!(
            offset = format!("0x{:04X}", offset),
            value = format!("0x{:02X}", value),
            "writing memory"
        );

        // Send status command first (observed in all write sequences)
        self.send_status_sync()?;

        // Now send the write command
        let cmd = build_memory_write(offset, value);
        let response = self.send_command(&cmd)?;

        // Validate response echoes the command
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
    pub fn get_polling_rate(&mut self) -> Result<PollingRate> {
        debug!("getting polling rate");
        let byte = self.read_memory_byte(OFFSET_POLLING_RATE)?;
        match PollingRate::from_byte(byte) {
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
    pub fn get_lift_off_distance(&mut self) -> Result<LiftOffDistance> {
        debug!("getting lift-off distance");
        let byte = self.read_memory_byte(OFFSET_LIFT_OFF_DISTANCE)?;
        match LiftOffDistance::from_byte(byte) {
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
    pub fn get_sleep_timeout(&mut self) -> Result<u16> {
        debug!("getting sleep timeout");
        let byte = self.read_memory_byte(OFFSET_SLEEP_TIMEOUT)?;
        let seconds = (byte as u16) * 10;
        debug!(seconds, "sleep timeout retrieved");
        Ok(seconds)
    }

    /// Get angle snapping state.
    #[instrument(skip(self))]
    pub fn get_angle_snapping(&mut self) -> Result<bool> {
        debug!("getting angle snapping state");
        let byte = self.read_memory_byte(OFFSET_ANGLE_SNAPPING)?;
        let enabled = byte == 0x01;
        debug!(enabled, "angle snapping state retrieved");
        Ok(enabled)
    }

    /// Get ripple control state.
    #[instrument(skip(self))]
    pub fn get_ripple_control(&mut self) -> Result<bool> {
        debug!("getting ripple control state");
        let byte = self.read_memory_byte(OFFSET_RIPPLE_CONTROL)?;
        let enabled = byte == 0x01;
        debug!(enabled, "ripple control state retrieved");
        Ok(enabled)
    }

    /// Get high speed mode state.
    #[instrument(skip(self))]
    pub fn get_high_speed_mode(&mut self) -> Result<bool> {
        debug!("getting high speed mode state");
        let byte = self.read_memory_byte(OFFSET_HIGH_SPEED_MODE)?;
        let enabled = byte == 0x01;
        debug!(enabled, "high speed mode state retrieved");
        Ok(enabled)
    }

    /// Get long distance mode state.
    #[instrument(skip(self))]
    pub fn get_long_distance_mode(&mut self) -> Result<bool> {
        debug!("getting long distance mode state");
        let cmd = build_wireless_status_cmd();
        let response = self.send_command(&cmd)?;

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

        let enabled = response[6] == 0x01;
        debug!(enabled, "long distance mode state retrieved");
        Ok(enabled)
    }

    /// Get the full mouse configuration.
    #[instrument(skip(self))]
    pub fn get_config(&mut self) -> Result<MouseConfig> {
        info!("retrieving full mouse configuration");
        let config = MouseConfig {
            polling_rate: self.get_polling_rate()?,
            lift_off_distance: self.get_lift_off_distance()?,
            sleep_timeout_seconds: self.get_sleep_timeout()?,
            angle_snapping: self.get_angle_snapping()?,
            ripple_control: self.get_ripple_control()?,
            high_speed_mode: self.get_high_speed_mode()?,
            long_distance_mode: self.get_long_distance_mode()?,
        };
        info!("mouse configuration retrieved successfully");
        Ok(config)
    }

    /// Get battery status.
    #[instrument(skip(self))]
    pub fn get_battery(&mut self) -> Result<BatteryStatus> {
        debug!("getting battery status");
        let cmd = build_battery_cmd();
        let response = self.send_command(&cmd)?;

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

        // Bytes 8-9: battery voltage in mV (big-endian)
        let voltage_mv = u16::from_be_bytes([response[8], response[9]]);
        let percentage = voltage_to_percentage(voltage_mv);

        debug!(voltage_mv, percentage, "battery status retrieved");
        Ok(BatteryStatus {
            voltage_mv,
            percentage,
        })
    }

    /// Get firmware version information.
    #[instrument(skip(self))]
    pub fn get_firmware_info(&mut self) -> Result<FirmwareInfo> {
        debug!("getting firmware information");

        // Mouse firmware
        let mouse_cmd = build_mouse_firmware_cmd();
        let mouse_response = self.send_command(&mouse_cmd)?;

        let mouse_version = if mouse_response.len() >= 8
            && mouse_response[0] == 0x08
            && mouse_response[1] == CMD_MOUSE_FIRMWARE
        {
            let major = decode_bcd(mouse_response[6]);
            let minor = decode_bcd(mouse_response[7]);
            let version = format!("v{}.{}", major, minor);
            trace!(mouse_version = %version, "mouse firmware version parsed");
            version
        } else {
            trace!("could not parse mouse firmware version");
            "Unknown".to_string()
        };

        // Receiver firmware (only meaningful in wireless mode)
        let receiver_cmd = build_receiver_firmware_cmd();
        let receiver_response = self.send_command(&receiver_cmd)?;

        let receiver_version = match self.mode {
            ConnectionMode::Wireless => {
                if receiver_response.len() >= 8
                    && receiver_response[0] == 0x08
                    && receiver_response[1] == CMD_RECEIVER_FIRMWARE
                {
                    let major = decode_bcd(receiver_response[6]);
                    let minor = decode_bcd(receiver_response[7]);
                    let version = format!("v{}.{}", major, minor);
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

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Set the polling rate.
    #[instrument(skip(self))]
    pub fn set_polling_rate(&mut self, rate: PollingRate) -> Result<()> {
        info!(?rate, "setting polling rate");
        self.write_memory(OFFSET_POLLING_RATE, rate.to_byte())?;
        info!(?rate, "polling rate set successfully");
        Ok(())
    }

    /// Set the lift-off distance.
    #[instrument(skip(self))]
    pub fn set_lift_off_distance(&mut self, lod: LiftOffDistance) -> Result<()> {
        info!(?lod, "setting lift-off distance");
        self.write_memory(OFFSET_LIFT_OFF_DISTANCE, lod.to_byte())?;
        info!(?lod, "lift-off distance set successfully");
        Ok(())
    }

    /// Set the sleep timeout in seconds.
    ///
    /// Must be a multiple of 10. Maximum value is 2550 seconds.
    /// The value is written to both the primary and secondary memory locations.
    #[instrument(skip(self))]
    pub fn set_sleep_timeout(&mut self, seconds: u16) -> Result<()> {
        info!(seconds, "setting sleep timeout");

        if seconds > MAX_SLEEP_TIMEOUT_SECONDS {
            error!(
                seconds,
                max = MAX_SLEEP_TIMEOUT_SECONDS,
                "sleep timeout exceeds maximum"
            );
            return Err(MouseError::InvalidSleepTimeout(seconds));
        }

        let value = (seconds / 10) as u8;
        trace!(raw_value = value, "calculated raw timeout value");

        // Write to secondary location first (as observed in dumps)
        self.write_memory(OFFSET_SLEEP_TIMEOUT_SECONDARY, value)?;
        // Then write to primary location
        self.write_memory(OFFSET_SLEEP_TIMEOUT, value)?;

        info!(seconds, "sleep timeout set successfully");
        Ok(())
    }

    /// Set angle snapping state.
    #[instrument(skip(self))]
    pub fn set_angle_snapping(&mut self, enabled: bool) -> Result<()> {
        info!(enabled, "setting angle snapping");
        self.write_memory(OFFSET_ANGLE_SNAPPING, if enabled { 0x01 } else { 0x00 })?;
        info!(enabled, "angle snapping set successfully");
        Ok(())
    }

    /// Set ripple control state.
    #[instrument(skip(self))]
    pub fn set_ripple_control(&mut self, enabled: bool) -> Result<()> {
        info!(enabled, "setting ripple control");
        self.write_memory(OFFSET_RIPPLE_CONTROL, if enabled { 0x01 } else { 0x00 })?;
        info!(enabled, "ripple control set successfully");
        Ok(())
    }

    /// Set high speed mode state.
    #[instrument(skip(self))]
    pub fn set_high_speed_mode(&mut self, enabled: bool) -> Result<()> {
        info!(enabled, "setting high speed mode");
        self.write_memory(OFFSET_HIGH_SPEED_MODE, if enabled { 0x01 } else { 0x00 })?;
        info!(enabled, "high speed mode set successfully");
        Ok(())
    }

    /// Set long distance mode state.
    ///
    /// This uses a special command (0x16) instead of memory write.
    #[instrument(skip(self))]
    pub fn set_long_distance_mode(&mut self, enabled: bool) -> Result<()> {
        info!(enabled, "setting long distance mode");

        // Send status command first (observed in all write sequences)
        self.send_status_sync()?;

        let cmd = build_long_distance_cmd(enabled);
        let response = self.send_command(&cmd)?;

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
    pub fn set_config(&mut self, config: &MouseConfig) -> Result<()> {
        info!("applying full mouse configuration");
        debug!(?config, "configuration to apply");

        self.set_polling_rate(config.polling_rate)?;
        self.set_lift_off_distance(config.lift_off_distance)?;
        self.set_sleep_timeout(config.sleep_timeout_seconds)?;
        self.set_angle_snapping(config.angle_snapping)?;
        self.set_ripple_control(config.ripple_control)?;
        self.set_high_speed_mode(config.high_speed_mode)?;
        self.set_long_distance_mode(config.long_distance_mode)?;

        info!("mouse configuration applied successfully");
        Ok(())
    }
}
