//! Mouse communication and configuration API.

use std::time::Duration;

use nusb::transfer::{Buffer, ControlOut, ControlType, In, Interrupt, Recipient};
use nusb::{Endpoint, Interface, MaybeFuture};

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
    pub fn open() -> Result<Self> {
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
            .ok_or_else(|| MouseError::NotFound {
                vid: VENDOR_ID,
                pids: PRODUCT_IDS.to_vec(),
            })?;

        let device = device_info.open().wait()?;

        // Detach kernel driver and claim interface
        let interface = device.detach_and_claim_interface(INTERFACE_NUM).wait()?;

        // Create interrupt endpoint for responses
        let endpoint = interface.endpoint::<Interrupt, In>(INTERRUPT_EP_IN)?;

        Ok(Mouse {
            interface,
            endpoint,
            mode,
        })
    }

    /// Get the connection mode (wired or wireless).
    pub fn connection_mode(&self) -> ConnectionMode {
        self.mode
    }

    // =========================================================================
    // Low-level Communication
    // =========================================================================

    /// Send a command and receive the response.
    fn send_command(&mut self, cmd: &[u8; 17]) -> Result<Vec<u8>> {
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
                        }
                    } else {
                        break;
                    }
                }
                None => {
                    if i == 0 {
                        return Err(MouseError::Timeout);
                    }
                    break;
                }
            }
        }

        if bytes_read == 0 {
            return Err(MouseError::Timeout);
        }

        Ok(response[..bytes_read].to_vec())
    }

    /// Send a status command (used before writes to sync with device).
    fn send_status_sync(&mut self) -> Result<()> {
        let cmd = build_status_cmd();
        let _ = self.send_command(&cmd)?;
        Ok(())
    }

    /// Read memory from the mouse.
    fn read_memory(&mut self, offset: u16, length: u8) -> Result<Vec<u8>> {
        let cmd = build_memory_read(offset, length);
        let response = self.send_command(&cmd)?;

        // Validate response
        if response.len() < 6 + length as usize {
            return Err(MouseError::InsufficientData {
                need: 6 + length as usize,
                got: response.len(),
            });
        }

        if response[1] != CMD_MEMORY_READ {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_MEMORY_READ,
                got: response[1],
            });
        }

        // Extract data bytes (starting at byte 6)
        Ok(response[6..6 + length as usize].to_vec())
    }

    /// Read a single byte from memory.
    fn read_memory_byte(&mut self, offset: u16) -> Result<u8> {
        let data = self.read_memory(offset, 1)?;
        Ok(data[0])
    }

    /// Write a single byte to memory.
    ///
    /// Follows the observed protocol pattern: send status command first, then write.
    fn write_memory(&mut self, offset: u16, value: u8) -> Result<()> {
        // Send status command first (observed in all write sequences)
        self.send_status_sync()?;

        // Now send the write command
        let cmd = build_memory_write(offset, value);
        let response = self.send_command(&cmd)?;

        // Validate response echoes the command
        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_MEMORY_WRITE {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_MEMORY_WRITE,
                got: response[1],
            });
        }

        Ok(())
    }

    // =========================================================================
    // Read Operations
    // =========================================================================

    /// Get the current polling rate.
    pub fn get_polling_rate(&mut self) -> Result<PollingRate> {
        let byte = self.read_memory_byte(OFFSET_POLLING_RATE)?;
        PollingRate::from_byte(byte).ok_or(MouseError::InvalidPollingRate(byte))
    }

    /// Get the current lift-off distance.
    pub fn get_lift_off_distance(&mut self) -> Result<LiftOffDistance> {
        let byte = self.read_memory_byte(OFFSET_LIFT_OFF_DISTANCE)?;
        LiftOffDistance::from_byte(byte).ok_or(MouseError::InvalidLiftOffDistance(byte))
    }

    /// Get the current sleep timeout in seconds.
    pub fn get_sleep_timeout(&mut self) -> Result<u16> {
        let byte = self.read_memory_byte(OFFSET_SLEEP_TIMEOUT)?;
        Ok((byte as u16) * 10)
    }

    /// Get angle snapping state.
    pub fn get_angle_snapping(&mut self) -> Result<bool> {
        let byte = self.read_memory_byte(OFFSET_ANGLE_SNAPPING)?;
        Ok(byte == 0x01)
    }

    /// Get ripple control state.
    pub fn get_ripple_control(&mut self) -> Result<bool> {
        let byte = self.read_memory_byte(OFFSET_RIPPLE_CONTROL)?;
        Ok(byte == 0x01)
    }

    /// Get high speed mode state.
    pub fn get_high_speed_mode(&mut self) -> Result<bool> {
        let byte = self.read_memory_byte(OFFSET_HIGH_SPEED_MODE)?;
        Ok(byte == 0x01)
    }

    /// Get long distance mode state.
    pub fn get_long_distance_mode(&mut self) -> Result<bool> {
        let cmd = build_wireless_status_cmd();
        let response = self.send_command(&cmd)?;

        if response.len() < 7 {
            return Err(MouseError::InsufficientData {
                need: 7,
                got: response.len(),
            });
        }

        Ok(response[6] == 0x01)
    }

    /// Get the full mouse configuration.
    pub fn get_config(&mut self) -> Result<MouseConfig> {
        Ok(MouseConfig {
            polling_rate: self.get_polling_rate()?,
            lift_off_distance: self.get_lift_off_distance()?,
            sleep_timeout_seconds: self.get_sleep_timeout()?,
            angle_snapping: self.get_angle_snapping()?,
            ripple_control: self.get_ripple_control()?,
            high_speed_mode: self.get_high_speed_mode()?,
            long_distance_mode: self.get_long_distance_mode()?,
        })
    }

    /// Get battery status.
    pub fn get_battery(&mut self) -> Result<BatteryStatus> {
        let cmd = build_battery_cmd();
        let response = self.send_command(&cmd)?;

        if response.len() < 10 {
            return Err(MouseError::InsufficientData {
                need: 10,
                got: response.len(),
            });
        }

        if response[1] != CMD_BATTERY {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_BATTERY,
                got: response[1],
            });
        }

        // Bytes 8-9: battery voltage in mV (big-endian)
        let voltage_mv = u16::from_be_bytes([response[8], response[9]]);
        let percentage = voltage_to_percentage(voltage_mv);

        Ok(BatteryStatus {
            voltage_mv,
            percentage,
        })
    }

    /// Get firmware version information.
    pub fn get_firmware_info(&mut self) -> Result<FirmwareInfo> {
        // Mouse firmware
        let mouse_cmd = build_mouse_firmware_cmd();
        let mouse_response = self.send_command(&mouse_cmd)?;

        let mouse_version = if mouse_response.len() >= 8
            && mouse_response[0] == 0x08
            && mouse_response[1] == CMD_MOUSE_FIRMWARE
        {
            let major = decode_bcd(mouse_response[6]);
            let minor = decode_bcd(mouse_response[7]);
            format!("v{}.{}", major, minor)
        } else {
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
                    Some(format!("v{}.{}", major, minor))
                } else {
                    None
                }
            }
            ConnectionMode::Wired => None,
        };

        Ok(FirmwareInfo {
            mouse_version,
            receiver_version,
        })
    }

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Set the polling rate.
    pub fn set_polling_rate(&mut self, rate: PollingRate) -> Result<()> {
        self.write_memory(OFFSET_POLLING_RATE, rate.to_byte())
    }

    /// Set the lift-off distance.
    pub fn set_lift_off_distance(&mut self, lod: LiftOffDistance) -> Result<()> {
        self.write_memory(OFFSET_LIFT_OFF_DISTANCE, lod.to_byte())
    }

    /// Set the sleep timeout in seconds.
    ///
    /// Must be a multiple of 10. Maximum value is 2550 seconds.
    /// The value is written to both the primary and secondary memory locations.
    pub fn set_sleep_timeout(&mut self, seconds: u16) -> Result<()> {
        if seconds > MAX_SLEEP_TIMEOUT_SECONDS {
            return Err(MouseError::InvalidSleepTimeout(seconds));
        }

        let value = (seconds / 10) as u8;

        // Write to secondary location first (as observed in dumps)
        self.write_memory(OFFSET_SLEEP_TIMEOUT_SECONDARY, value)?;
        // Then write to primary location
        self.write_memory(OFFSET_SLEEP_TIMEOUT, value)?;

        Ok(())
    }

    /// Set angle snapping state.
    pub fn set_angle_snapping(&mut self, enabled: bool) -> Result<()> {
        self.write_memory(OFFSET_ANGLE_SNAPPING, if enabled { 0x01 } else { 0x00 })
    }

    /// Set ripple control state.
    pub fn set_ripple_control(&mut self, enabled: bool) -> Result<()> {
        self.write_memory(OFFSET_RIPPLE_CONTROL, if enabled { 0x01 } else { 0x00 })
    }

    /// Set high speed mode state.
    pub fn set_high_speed_mode(&mut self, enabled: bool) -> Result<()> {
        self.write_memory(OFFSET_HIGH_SPEED_MODE, if enabled { 0x01 } else { 0x00 })
    }

    /// Set long distance mode state.
    ///
    /// This uses a special command (0x16) instead of memory write.
    pub fn set_long_distance_mode(&mut self, enabled: bool) -> Result<()> {
        // Send status command first (observed in all write sequences)
        self.send_status_sync()?;

        let cmd = build_long_distance_cmd(enabled);
        let response = self.send_command(&cmd)?;

        if response.len() < 2 {
            return Err(MouseError::InsufficientData {
                need: 2,
                got: response.len(),
            });
        }

        if response[1] != CMD_LONG_DISTANCE {
            return Err(MouseError::UnexpectedResponse {
                expected: CMD_LONG_DISTANCE,
                got: response[1],
            });
        }

        Ok(())
    }

    /// Apply a full configuration.
    pub fn set_config(&mut self, config: &MouseConfig) -> Result<()> {
        self.set_polling_rate(config.polling_rate)?;
        self.set_lift_off_distance(config.lift_off_distance)?;
        self.set_sleep_timeout(config.sleep_timeout_seconds)?;
        self.set_angle_snapping(config.angle_snapping)?;
        self.set_ripple_control(config.ripple_control)?;
        self.set_high_speed_mode(config.high_speed_mode)?;
        self.set_long_distance_mode(config.long_distance_mode)?;
        Ok(())
    }
}
