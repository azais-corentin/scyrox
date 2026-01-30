//! USB protocol constants and packet building functions.

// =============================================================================
// USB Device Identifiers
// =============================================================================

/// Vendor ID for the mouse.
pub const VENDOR_ID: u16 = 0x3554;

/// Product ID for wired mode.
pub const PID_WIRED: u16 = 0xF5F6;

/// Product ID for 4K wireless dongle.
pub const PID_WIRELESS_4K: u16 = 0xF5F7;

/// Product ID for standard wireless dongle.
pub const PID_WIRELESS_STD: u16 = 0xF5F4;

/// Supported Product IDs (preferred first: wired, then 4K wireless, then standard wireless).
pub const PRODUCT_IDS: [u16; 3] = [PID_WIRED, PID_WIRELESS_4K, PID_WIRELESS_STD];

/// USB interface number for configuration.
pub const INTERFACE_NUM: u8 = 1;

/// Interrupt endpoint for reading responses.
pub const INTERRUPT_EP_IN: u8 = 0x82;

/// Packet size for wired mode (from USB descriptors).
pub const PACKET_SIZE_WIRED: usize = 64;

/// Packet size for wireless mode (from USB descriptors).
pub const PACKET_SIZE_WIRELESS: usize = 49;

// =============================================================================
// Command Codes (per protocol spec)
// =============================================================================

/// EncryptionData (0x01) - Device handshake/identification.
pub const CMD_ENCRYPTION_DATA: u8 = 0x01;
/// Alias for backward compatibility.
pub const CMD_DEVICE_INFO: u8 = CMD_ENCRYPTION_DATA;

/// PCDriverStatus (0x02) - Notify device of driver connection.
pub const CMD_PC_DRIVER_STATUS: u8 = 0x02;
/// Alias for backward compatibility.
pub const CMD_CONFIG_FLAGS: u8 = CMD_PC_DRIVER_STATUS;

/// DeviceOnLine (0x03) - Check if mouse is connected to dongle.
pub const CMD_DEVICE_ONLINE: u8 = 0x03;
/// Alias for backward compatibility.
pub const CMD_STATUS: u8 = CMD_DEVICE_ONLINE;

/// BatteryLevel (0x04) - Get battery status.
pub const CMD_BATTERY_LEVEL: u8 = 0x04;
/// Alias for backward compatibility.
pub const CMD_BATTERY: u8 = CMD_BATTERY_LEVEL;

/// WriteFlashData (0x07) - Write to flash memory.
pub const CMD_WRITE_FLASH: u8 = 0x07;
/// Alias for backward compatibility.
pub const CMD_MEMORY_WRITE: u8 = CMD_WRITE_FLASH;

/// ReadFlashData (0x08) - Read from flash memory.
pub const CMD_READ_FLASH: u8 = 0x08;
/// Alias for backward compatibility.
pub const CMD_MEMORY_READ: u8 = CMD_READ_FLASH;

/// ReadVersionID (0x12) - Get mouse firmware version.
pub const CMD_READ_VERSION: u8 = 0x12;
/// Alias for backward compatibility.
pub const CMD_MOUSE_FIRMWARE: u8 = CMD_READ_VERSION;

/// SetLongRangeMode (0x16) - Enable/disable long range mode.
pub const CMD_SET_LONG_RANGE: u8 = 0x16;
/// Alias for backward compatibility.
pub const CMD_LONG_DISTANCE: u8 = CMD_SET_LONG_RANGE;

/// GetLongRangeMode (0x17) - Query long range mode status.
pub const CMD_GET_LONG_RANGE: u8 = 0x17;
/// Alias for backward compatibility.
pub const CMD_WIRELESS_STATUS: u8 = CMD_GET_LONG_RANGE;

/// DongleEnterPair (0x05) - Enter pairing mode.
pub const CMD_DONGLE_ENTER_PAIR: u8 = 0x05;

/// GetPairState (0x06) - Query pairing status.
pub const CMD_GET_PAIR_STATE: u8 = 0x06;

/// ClearSetting (0x09) - Factory reset.
pub const CMD_CLEAR_SETTING: u8 = 0x09;

/// StatusChanged (0x0A) - Configuration change notification (unsolicited from device).
pub const CMD_STATUS_CHANGED: u8 = 0x0A;

/// GetCurrentConfig (0x0E) - Get active profile index.
pub const CMD_GET_CURRENT_CONFIG: u8 = 0x0E;

/// SetCurrentConfig (0x0F) - Set active profile.
pub const CMD_SET_CURRENT_CONFIG: u8 = 0x0F;

/// GetDongleVersion (0x1D) - Get dongle firmware version.
pub const CMD_DONGLE_VERSION: u8 = 0x1D;
/// Alias for backward compatibility.
pub const CMD_RECEIVER_FIRMWARE: u8 = CMD_DONGLE_VERSION;

// =============================================================================
// Memory Offsets (for read/write commands)
// =============================================================================

/// Report rate (1 byte) - address 0x0000.
pub const OFFSET_REPORT_RATE: u16 = 0x0000;
/// Alias for backward compatibility.
pub const OFFSET_POLLING_RATE: u16 = OFFSET_REPORT_RATE;

/// Max DPI count (1 byte) - address 0x0002.
pub const OFFSET_MAX_DPI: u16 = 0x0002;

/// Current DPI index (1 byte) - address 0x0004.
pub const OFFSET_CURRENT_DPI: u16 = 0x0004;

/// 20K Sensor Mode (1 byte) - address 0x0008.
pub const OFFSET_SENSOR_20K: u16 = 0x0008;

/// Lift-off distance (1 byte) - address 0x000A.
pub const OFFSET_LIFT_OFF_DISTANCE: u16 = 0x000A;

/// DPI Values base address (8 stages × 4 bytes = 32 bytes) - address 0x000C-0x002B.
pub const OFFSET_DPI_VALUES: u16 = 0x000C;

/// DPI Colors base address (8 stages × 4 bytes = 32 bytes) - address 0x002C-0x004B.
pub const OFFSET_DPI_COLORS: u16 = 0x002C;

/// DPI Effect Mode (1 byte) - address 0x004C.
pub const OFFSET_DPI_EFFECT_MODE: u16 = 0x004C;

/// DPI Effect Brightness (1 byte) - address 0x004E.
pub const OFFSET_DPI_EFFECT_BRIGHTNESS: u16 = 0x004E;

/// DPI Effect Speed (1 byte) - address 0x0050.
pub const OFFSET_DPI_EFFECT_SPEED: u16 = 0x0050;

/// DPI Effect State (1 byte) - address 0x0052.
pub const OFFSET_DPI_EFFECT_STATE: u16 = 0x0052;

/// Key Functions base address (8 keys × 4 bytes = 32 bytes) - address 0x0060-0x007F.
pub const OFFSET_KEY_FUNCTIONS: u16 = 0x0060;

/// Light Settings base address (7 bytes) - address 0x00A0-0x00A6.
pub const OFFSET_LIGHT_SETTINGS: u16 = 0x00A0;

/// Light On/Off State (1 byte) - address 0x00A7.
pub const OFFSET_LIGHT_STATE: u16 = 0x00A7;

/// Debounce Time (1 byte) - address 0x00A9.
pub const OFFSET_DEBOUNCE_TIME: u16 = 0x00A9;

/// Motion Sync (1 byte) - address 0x00AB.
pub const OFFSET_MOTION_SYNC: u16 = 0x00AB;

/// Sleep Time (1 byte) - address 0x00AD.
pub const OFFSET_SLEEP_TIME: u16 = 0x00AD;
/// Alias for backward compatibility.
pub const OFFSET_SLEEP_TIMEOUT: u16 = OFFSET_SLEEP_TIME;

/// Angle Snapping (1 byte) - address 0x00AF.
pub const OFFSET_ANGLE_SNAPPING: u16 = 0x00AF;

/// Ripple Control (1 byte) - address 0x00B1.
pub const OFFSET_RIPPLE_CONTROL: u16 = 0x00B1;

/// Moving Off Light Time (1 byte) - address 0x00B3.
pub const OFFSET_MOVING_OFF_LIGHT: u16 = 0x00B3;

/// Performance State / High Speed Mode (1 byte) - address 0x00B5.
pub const OFFSET_PERFORMANCE_STATE: u16 = 0x00B5;
/// Alias for backward compatibility.
pub const OFFSET_HIGH_SPEED_MODE: u16 = OFFSET_PERFORMANCE_STATE;

/// Performance/Sleep Time Value (1 byte) - address 0x00B7.
pub const OFFSET_PERFORMANCE_TIME: u16 = 0x00B7;
/// Alias for backward compatibility.
pub const OFFSET_SLEEP_TIMEOUT_SECONDARY: u16 = OFFSET_PERFORMANCE_TIME;

/// Sensor Mode (1 byte) - address 0x00B9.
pub const OFFSET_SENSOR_MODE: u16 = 0x00B9;

/// Shortcut Keys base address (8 slots × 32 bytes = 256 bytes) - address 0x0100-0x01FF.
pub const OFFSET_SHORTCUT_KEYS: u16 = 0x0100;

/// Macros base address (8 slots × 384 bytes = 3072 bytes) - address 0x0300-0x0BFF.
pub const OFFSET_MACROS: u16 = 0x0300;

// =============================================================================
// Shortcut Key Event Flags
// =============================================================================

/// Shortcut key event flag: Key down (bit 7).
pub const SHORTCUT_KEY_DOWN: u8 = 0x80;

/// Shortcut key event flag: Key up (bit 6).
pub const SHORTCUT_KEY_UP: u8 = 0x40;

/// Size of a shortcut key slot in bytes.
pub const SHORTCUT_KEY_SLOT_SIZE: usize = 32;

/// Maximum events per shortcut key.
pub const SHORTCUT_KEY_MAX_EVENTS: usize = 10;

// =============================================================================
// Macro Event Flags
// =============================================================================

/// Macro event status: Key down (bits 6-7 = 01).
pub const MACRO_EVENT_KEY_DOWN: u8 = 0x40;

/// Macro event status: Key up (bits 6-7 = 10).
pub const MACRO_EVENT_KEY_UP: u8 = 0x80;

/// Macro event key type: Keyboard (bits 0-3 = 1).
pub const MACRO_KEY_TYPE_KEYBOARD: u8 = 0x01;

/// Macro event key type: Mouse button (bits 0-3 = 4).
pub const MACRO_KEY_TYPE_MOUSE: u8 = 0x04;

/// Size of a macro slot in bytes.
pub const MACRO_SLOT_SIZE: usize = 384;

/// Maximum name length for a macro.
pub const MACRO_NAME_MAX_LENGTH: usize = 30;

/// Maximum events per macro.
pub const MACRO_MAX_EVENTS: usize = 70;

/// Size of a macro event in bytes.
pub const MACRO_EVENT_SIZE: usize = 5;

// =============================================================================
// HID Report Configuration
// =============================================================================

/// HID Report ID used for all communication.
pub const REPORT_ID: u8 = 8;

/// Packet length (excluding report ID which is handled by HID layer).
pub const PACKET_LENGTH: usize = 16;

// =============================================================================
// Checksum Functions
// =============================================================================

/// Calculate packet checksum.
///
/// Formula: checksum = (0x55 - sum(bytes 0-14) - REPORT_ID) & 0xFF
/// The checksum is placed at byte 15.
pub fn calculate_checksum(packet: &[u8; PACKET_LENGTH]) -> u8 {
    let sum: u16 = packet[0..15].iter().map(|&b| b as u16).sum();
    let truncated = (sum & 0xFF) as u8;
    0x55u8.wrapping_sub(truncated).wrapping_sub(REPORT_ID)
}

/// Calculate data checksum for single-byte write commands.
///
/// For single-byte writes, the complement is: 0x55 - value
pub fn calculate_data_checksum(value: u8) -> u8 {
    0x55u8.wrapping_sub(value)
}

/// Verify the checksum of a response packet.
///
/// Per protocol spec, the checksum is at byte 15 and is calculated the same way
/// as for outgoing packets: (0x55 - sum(bytes 0-14) - REPORT_ID) & 0xFF
///
/// Note: The response packet should NOT include the report ID prefix when passed
/// to this function (it should be 16 bytes starting with the command ID).
pub fn verify_response_checksum(response: &[u8]) -> bool {
    if response.len() < PACKET_LENGTH {
        return false;
    }
    let sum: u16 = response[0..15].iter().map(|&b| b as u16).sum();
    let expected = 0x55u8
        .wrapping_sub((sum & 0xFF) as u8)
        .wrapping_sub(REPORT_ID);
    response[15] == expected
}

/// Validate that a response matches the request per protocol spec section 8.4.
///
/// For ReadFlashData (0x08) commands, verifies that bytes 0-4 of the response
/// match the request (command, reserved, address high, address low, length).
///
/// For all other commands, verifies that bytes 0-2 match (command, reserved, address high).
///
/// Note: Both request and response should be 16-byte packets (without report ID prefix).
pub fn validate_response(request: &[u8; PACKET_LENGTH], response: &[u8]) -> bool {
    if response.len() < 5 {
        return false;
    }

    if request[0] == CMD_READ_FLASH {
        // ReadFlashData: verify bytes 0-4 match
        request[0..5] == response[0..5]
    } else {
        // Other commands: verify bytes 0-2 match (command, reserved, address high)
        // Note: We only check command and status byte for most commands
        // since reserved byte and address may not be echoed by all commands
        request[0] == response[0]
    }
}

// =============================================================================
// Packet Building Functions
// =============================================================================

/// Build a generic command packet (16 bytes).
///
/// Packet format (per protocol spec):
/// - Byte 0: Command ID
/// - Byte 1: Reserved (always 0x00)
/// - Byte 2: Address high byte (for flash operations)
/// - Byte 3: Address low byte (for flash operations)
/// - Byte 4: Data length (number of data bytes, max 10)
/// - Bytes 5-14: Data payload
/// - Byte 15: Checksum
///
/// Note: Report ID (8) is prepended by the HID layer, not included in packet.
pub fn build_command(cmd: u8, address: u16, length: u8) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = cmd;
    packet[1] = 0x00; // Reserved
    packet[2] = (address >> 8) as u8; // Address HIGH byte
    packet[3] = (address & 0xFF) as u8; // Address LOW byte
    packet[4] = length;
    // Bytes 5-14 are zero (data payload)
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build a simple command with no address or data.
pub fn build_simple_command(cmd: u8) -> [u8; PACKET_LENGTH] {
    build_command(cmd, 0, 0)
}

/// Build memory read command (cmd 0x08).
pub fn build_memory_read(address: u16, length: u8) -> [u8; PACKET_LENGTH] {
    build_command(CMD_MEMORY_READ, address, length)
}

/// Build memory write command (cmd 0x07) for a single byte value.
///
/// Write format uses 2 data bytes: value + complement (0x55 - value).
pub fn build_memory_write(address: u16, value: u8) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_MEMORY_WRITE;
    packet[1] = 0x00;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = 0x02; // Length: value byte + complement byte
    packet[5] = value;
    packet[6] = calculate_data_checksum(value);
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build device info/handshake command (cmd 0x01 - EncryptionData).
pub fn build_handshake_cmd(random_bytes: &[u8; 4]) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_DEVICE_INFO;
    packet[1] = 0x00;
    packet[2] = 0x00;
    packet[3] = 0x00;
    packet[4] = 0x08; // Data length
    packet[5..9].copy_from_slice(random_bytes);
    // Bytes 9-14 are zero
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build status command (cmd 0x03 - DeviceOnLine).
pub fn build_status_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_STATUS)
}

/// Build battery status command (cmd 0x04).
pub fn build_battery_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_BATTERY)
}

/// Build mouse firmware version command (cmd 0x12).
pub fn build_mouse_firmware_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_MOUSE_FIRMWARE)
}

/// Build receiver/dongle firmware version command (cmd 0x1d).
pub fn build_receiver_firmware_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_RECEIVER_FIRMWARE)
}

/// Build get long range mode command (cmd 0x17).
pub fn build_get_long_range_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_WIRELESS_STATUS)
}

/// Build PC driver status command (cmd 0x02).
///
/// Notifies the device that a driver is connected/disconnected.
pub fn build_pc_driver_status_cmd(connected: bool) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_CONFIG_FLAGS;
    packet[1] = 0x00;
    packet[4] = 0x01; // Data length
    packet[5] = if connected { 0x01 } else { 0x00 };
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build set long range mode command (cmd 0x16).
pub fn build_set_long_range_cmd(enabled: bool) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_LONG_DISTANCE;
    packet[1] = 0x00;
    packet[2] = 0x00;
    packet[3] = 0x00;
    packet[4] = 0x0a; // Data length = 10
    packet[5] = if enabled { 0x01 } else { 0x00 };
    // Bytes 6-14 are zero
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build dongle enter pair command (cmd 0x05).
///
/// Puts the dongle into pairing mode to accept a new mouse.
/// Timeout is 62 seconds (0x3E).
pub fn build_dongle_enter_pair_cmd() -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_DONGLE_ENTER_PAIR;
    packet[1] = 0x00;
    packet[4] = 0x02; // Data length
    packet[5] = 0x00;
    packet[6] = 0x00;
    packet[7] = 0x3E; // 62 seconds timeout
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build get pair state command (cmd 0x06).
pub fn build_get_pair_state_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_GET_PAIR_STATE)
}

/// Build clear setting/factory reset command (cmd 0x09).
pub fn build_clear_setting_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_CLEAR_SETTING)
}

/// Build get current config/profile command (cmd 0x0E).
pub fn build_get_current_config_cmd() -> [u8; PACKET_LENGTH] {
    build_simple_command(CMD_GET_CURRENT_CONFIG)
}

/// Build set current config/profile command (cmd 0x0F).
pub fn build_set_current_config_cmd(profile_index: u8) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_SET_CURRENT_CONFIG;
    packet[1] = 0x00;
    packet[4] = 0x01; // Data length
    packet[5] = profile_index;
    packet[15] = calculate_checksum(&packet);
    packet
}

/// Build memory write command (cmd 0x07) for multiple bytes.
///
/// Can write up to 10 bytes of data.
pub fn build_flash_write(address: u16, data: &[u8]) -> [u8; PACKET_LENGTH] {
    let mut packet = [0u8; PACKET_LENGTH];
    packet[0] = CMD_WRITE_FLASH;
    packet[1] = 0x00;
    packet[2] = (address >> 8) as u8;
    packet[3] = (address & 0xFF) as u8;
    packet[4] = data.len().min(10) as u8;
    for (i, &byte) in data.iter().enumerate().take(10) {
        packet[5 + i] = byte;
    }
    packet[15] = calculate_checksum(&packet);
    packet
}

// =============================================================================
// DPI Encoding/Decoding Functions
// =============================================================================

/// Minimum DPI value.
pub const DPI_MIN: u16 = 50;

/// Maximum DPI value.
pub const DPI_MAX: u16 = 26000;

/// DPI step size.
pub const DPI_STEP: u16 = 50;

/// Encode a DPI value to 4-byte wire format.
///
/// Per protocol spec:
/// ```text
/// Offset 0: DPI low byte (value / 50 - 1) & 0xFF
/// Offset 1: DPI low byte (duplicate)
/// Offset 2: High bits: ((value / 50 - 1) >> 8) << 2 | ((value / 50 - 1) >> 8) << 6
/// Offset 3: Checksum of bytes 0-2
/// ```
pub fn encode_dpi(dpi: u16) -> [u8; 4] {
    let encoded = (dpi / DPI_STEP).saturating_sub(1);
    let low = (encoded & 0xFF) as u8;
    let high = ((encoded >> 8) & 0x03) as u8;
    let byte2 = (high << 2) | (high << 6);
    let checksum = low.wrapping_add(low).wrapping_add(byte2);
    let checksum = 0x55u8.wrapping_sub(checksum);
    [low, low, byte2, checksum]
}

/// Decode a 4-byte DPI value from wire format.
///
/// Per protocol spec:
/// ```text
/// high_bits = ((bytes[2] & 0x0C) >> 2)
/// value = bytes[0] | (high_bits << 8)
/// dpi = (value + 1) * 50
/// ```
pub fn decode_dpi(bytes: &[u8; 4]) -> u16 {
    let high_bits = ((bytes[2] & 0x0C) >> 2) as u16;
    let value = (bytes[0] as u16) | (high_bits << 8);
    (value + 1) * DPI_STEP
}

/// Encode a report rate (Hz) to wire byte value.
///
/// Per protocol spec:
/// - Values <= 1000 Hz: 1000 / hz
/// - Values > 1000 Hz: (hz / 2000) * 16
pub fn encode_report_rate(hz: u16) -> u8 {
    if hz <= 1000 {
        (1000 / hz) as u8
    } else {
        ((hz / 2000) * 16) as u8
    }
}

/// Decode a report rate byte value to Hz.
///
/// Per protocol spec:
/// - Values >= 16: (value / 16) * 2000
/// - Values < 16: 1000 / value
pub fn decode_report_rate(value: u8) -> u16 {
    if value >= 16 {
        (value as u16 / 16) * 2000
    } else if value > 0 {
        1000 / value as u16
    } else {
        1000 // Default
    }
}

// =============================================================================
// Response Parsing Utilities
// =============================================================================

/// Voltage to percentage lookup table (millivolts -> percentage).
/// Per protocol spec section 5.4.
const VOLTAGE_TABLE: [(u16, u8); 21] = [
    (3050, 0),
    (3420, 5),
    (3480, 10),
    (3540, 15),
    (3600, 20),
    (3660, 25),
    (3720, 30),
    (3760, 35),
    (3800, 40),
    (3840, 45),
    (3880, 50),
    (3920, 55),
    (3940, 60),
    (3960, 65),
    (3980, 70),
    (4000, 75),
    (4020, 80),
    (4040, 85),
    (4060, 90),
    (4080, 95),
    (4110, 100),
];

/// Convert battery voltage (mV) to percentage using the lookup table.
///
/// Per protocol spec section 5.4, uses linear interpolation between table values.
pub fn voltage_to_percentage_table(voltage_mv: u16) -> u8 {
    // Handle edge cases
    if voltage_mv <= VOLTAGE_TABLE[0].0 {
        return VOLTAGE_TABLE[0].1;
    }
    if voltage_mv >= VOLTAGE_TABLE[VOLTAGE_TABLE.len() - 1].0 {
        return VOLTAGE_TABLE[VOLTAGE_TABLE.len() - 1].1;
    }

    // Find the appropriate range and interpolate
    for i in 1..VOLTAGE_TABLE.len() {
        if voltage_mv <= VOLTAGE_TABLE[i].0 {
            let (v0, p0) = VOLTAGE_TABLE[i - 1];
            let (v1, p1) = VOLTAGE_TABLE[i];
            // Linear interpolation
            let ratio = (voltage_mv - v0) as f32 / (v1 - v0) as f32;
            let percentage = p0 as f32 + ratio * (p1 - p0) as f32;
            return percentage.round() as u8;
        }
    }

    100 // Should not reach here
}

/// Decode a BCD (Binary Coded Decimal) byte to its decimal value.
///
/// Example: 0x16 => 16, 0x22 => 22
pub fn decode_bcd(byte: u8) -> u8 {
    let high = (byte >> 4) & 0x0F;
    let low = byte & 0x0F;
    high * 10 + low
}

/// Convert battery voltage (mV) to percentage.
///
/// Uses a Li-ion discharge curve approximation.
/// Typical Li-ion: 3600mV ~= 0%, 4200mV ~= 100%
///
/// Note: `voltage_to_percentage_table()` uses the official lookup table from protocol spec.
pub fn voltage_to_percentage(voltage_mv: u16) -> u8 {
    // Formula: p = 123 - 123 / (1 + (v/3.7)^80)^0.165
    let v = voltage_mv as f32 / 1000.0;
    let denom = (1.0 + (v / 3.7).powi(80)).powf(0.165);
    let value = 123.0 - 123.0 / denom;
    let percent = value.round() as i32;
    percent.clamp(0, 100) as u8
}

/// Brightness level lookup table (index -> raw value).
/// Per protocol spec section 6.9.
const BRIGHTNESS_TABLE: [u8; 10] = [16, 30, 60, 90, 128, 150, 180, 210, 230, 255];

/// Encode brightness level (1-10) to raw value.
pub fn encode_brightness(level: u8) -> u8 {
    let index = (level.saturating_sub(1) as usize).min(BRIGHTNESS_TABLE.len() - 1);
    BRIGHTNESS_TABLE[index]
}

/// Decode raw brightness value to level (1-10).
pub fn decode_brightness(value: u8) -> u8 {
    for (i, &v) in BRIGHTNESS_TABLE.iter().enumerate() {
        if value <= v {
            return (i + 1) as u8;
        }
    }
    10
}

/// Format firmware version string per protocol spec.
///
/// Format: "v{major}.{minor:02x}"
/// Example: major=2, minor=0x20 → "v2.20"
pub fn format_firmware_version(major: u8, minor: u8) -> String {
    format!("v{}.{:02x}", major, minor)
}

// =============================================================================
// StatusChanged Notification Parsing
// =============================================================================

/// Parse a StatusChanged notification packet.
///
/// Per protocol spec section 5.10, this is an unsolicited notification sent by the device
/// when settings change (e.g., DPI button pressed on mouse).
///
/// Packet format (with report ID at byte 0):
/// - Byte 0: Report ID (0x08)
/// - Byte 1: Command ID (0x0A)
/// - Byte 2: Status (0x00)
/// - Byte 6: Change flags (bitmask)
///
/// Returns `Some(StatusChangeFlags)` if the packet is a valid StatusChanged notification,
/// `None` otherwise.
///
/// Note: The input packet should include the Report ID at byte 0.
pub fn parse_status_changed_notification(packet: &[u8]) -> Option<crate::types::StatusChangeFlags> {
    if packet.len() < 7 {
        return None;
    }

    // Check if this is a StatusChanged notification
    // Byte 0 is Report ID, Byte 1 is Command ID
    if packet[1] != CMD_STATUS_CHANGED {
        return None;
    }

    // Extract change flags from byte 6
    let flags = packet[6];
    Some(crate::types::StatusChangeFlags(flags))
}

/// Check if a packet is a StatusChanged notification.
///
/// This is useful for distinguishing unsolicited notifications from command responses.
///
/// Note: The input packet should include the Report ID at byte 0.
pub fn is_status_changed_notification(packet: &[u8]) -> bool {
    packet.len() >= 2 && packet[1] == CMD_STATUS_CHANGED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_calculation() {
        // Test with a memory write command for polling rate 125Hz (value 0x08)
        // Per protocol: checksum = (0x55 - sum(bytes 0-14) - REPORT_ID) & 0xFF
        let mut packet = [0u8; PACKET_LENGTH];
        packet[0] = CMD_MEMORY_WRITE; // 0x07
        packet[4] = 0x02; // Length
        packet[5] = 0x08; // Value
        packet[6] = 0x4d; // Complement (0x55 - 0x08)
                          // Sum of bytes 0-14: 0x07 + 0x02 + 0x08 + 0x4d = 0x5E
                          // Checksum: (0x55 - 0x5E - 0x08) & 0xFF = 0xEF
        assert_eq!(calculate_checksum(&packet), 0xef);
    }

    #[test]
    fn test_data_checksum_calculation() {
        // Data byte 0x08 should have complement 0x4d (0x55 - 0x08 = 0x4d)
        assert_eq!(calculate_data_checksum(0x08), 0x4d);
        // Data byte 0x01 should have complement 0x54
        assert_eq!(calculate_data_checksum(0x01), 0x54);
        // Data byte 0x00 should have complement 0x55
        assert_eq!(calculate_data_checksum(0x00), 0x55);
    }

    #[test]
    fn test_build_memory_write() {
        // Test building a polling rate write command for 125Hz
        let packet = build_memory_write(OFFSET_POLLING_RATE, 0x08);
        assert_eq!(packet[0], CMD_MEMORY_WRITE); // Command (0x07)
        assert_eq!(packet[1], 0x00); // Reserved
        assert_eq!(packet[2], 0x00); // Address high
        assert_eq!(packet[3], 0x00); // Address low
        assert_eq!(packet[4], 0x02); // Length
        assert_eq!(packet[5], 0x08); // Value
        assert_eq!(packet[6], 0x4d); // Data complement
        assert_eq!(packet[15], 0xef); // Packet checksum
    }

    #[test]
    fn test_build_set_long_range_cmd() {
        // Enable long range mode
        let packet = build_set_long_range_cmd(true);
        assert_eq!(packet[0], CMD_LONG_DISTANCE); // 0x16
        assert_eq!(packet[1], 0x00); // Reserved
        assert_eq!(packet[4], 0x0a); // Data length = 10
        assert_eq!(packet[5], 0x01); // Enabled

        // Disable long range mode
        let packet = build_set_long_range_cmd(false);
        assert_eq!(packet[5], 0x00); // Disabled
    }

    #[test]
    fn test_decode_bcd() {
        assert_eq!(decode_bcd(0x16), 16);
        assert_eq!(decode_bcd(0x22), 22);
        assert_eq!(decode_bcd(0x00), 0);
        assert_eq!(decode_bcd(0x99), 99);
    }

    #[test]
    fn test_voltage_to_percentage() {
        // Full battery (~4.2V)
        assert_eq!(voltage_to_percentage(4200), 100);
        // Empty battery (~3.6V)
        assert!(voltage_to_percentage(3600) < 20);
        // Very low
        assert_eq!(voltage_to_percentage(3000), 0);
        // Overcharged (cap at 100)
        assert_eq!(voltage_to_percentage(4500), 100);
    }

    #[test]
    fn test_voltage_to_percentage_table() {
        // Test values from the protocol spec lookup table
        assert_eq!(voltage_to_percentage_table(3050), 0);
        assert_eq!(voltage_to_percentage_table(3420), 5);
        assert_eq!(voltage_to_percentage_table(3880), 50);
        assert_eq!(voltage_to_percentage_table(4110), 100);
        // Below minimum
        assert_eq!(voltage_to_percentage_table(3000), 0);
        // Above maximum
        assert_eq!(voltage_to_percentage_table(4200), 100);
    }

    #[test]
    fn test_encode_decode_dpi() {
        // Test various DPI values
        let test_values = [50, 100, 400, 800, 1600, 3200, 6400, 12800, 26000];
        for &dpi in &test_values {
            let encoded = encode_dpi(dpi);
            let decoded = decode_dpi(&encoded);
            assert_eq!(decoded, dpi, "DPI {} round-trip failed", dpi);
        }
    }

    #[test]
    fn test_encode_decode_report_rate() {
        // Test standard rates
        assert_eq!(encode_report_rate(125), 8);
        assert_eq!(encode_report_rate(250), 4);
        assert_eq!(encode_report_rate(500), 2);
        assert_eq!(encode_report_rate(1000), 1);
        assert_eq!(encode_report_rate(2000), 16);
        assert_eq!(encode_report_rate(4000), 32);
        assert_eq!(encode_report_rate(8000), 64);

        // Test decoding
        assert_eq!(decode_report_rate(8), 125);
        assert_eq!(decode_report_rate(4), 250);
        assert_eq!(decode_report_rate(2), 500);
        assert_eq!(decode_report_rate(1), 1000);
        assert_eq!(decode_report_rate(16), 2000);
        assert_eq!(decode_report_rate(32), 4000);
        assert_eq!(decode_report_rate(64), 8000);
    }

    #[test]
    fn test_encode_decode_brightness() {
        // Test that encode/decode round-trips correctly
        for level in 1..=10 {
            let encoded = encode_brightness(level);
            let decoded = decode_brightness(encoded);
            assert_eq!(
                decoded, level,
                "Brightness level {} round-trip failed",
                level
            );
        }
    }

    #[test]
    fn test_format_firmware_version() {
        assert_eq!(format_firmware_version(2, 0x20), "v2.20");
        assert_eq!(format_firmware_version(1, 0x05), "v1.05");
        assert_eq!(format_firmware_version(3, 0x00), "v3.00");
    }

    #[test]
    fn test_build_set_current_config_cmd() {
        let packet = build_set_current_config_cmd(2);
        assert_eq!(packet[0], CMD_SET_CURRENT_CONFIG); // 0x0F
        assert_eq!(packet[4], 0x01); // Data length
        assert_eq!(packet[5], 0x02); // Profile index
    }

    #[test]
    fn test_build_flash_write() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let packet = build_flash_write(0x000C, &data);
        assert_eq!(packet[0], CMD_WRITE_FLASH); // 0x07
        assert_eq!(packet[2], 0x00); // Address high
        assert_eq!(packet[3], 0x0C); // Address low
        assert_eq!(packet[4], 0x04); // Length
        assert_eq!(packet[5], 0x12);
        assert_eq!(packet[6], 0x34);
        assert_eq!(packet[7], 0x56);
        assert_eq!(packet[8], 0x78);
    }

    #[test]
    fn test_verify_response_checksum() {
        // Build a valid packet and verify its checksum
        let packet = build_simple_command(CMD_BATTERY);
        assert!(verify_response_checksum(&packet));

        // Modify a byte and verify checksum fails
        let mut bad_packet = packet;
        bad_packet[5] = 0xFF;
        assert!(!verify_response_checksum(&bad_packet));

        // Test with packet that's too short
        let short_packet = [0u8; 10];
        assert!(!verify_response_checksum(&short_packet));
    }

    #[test]
    fn test_validate_response_read_flash() {
        // Create a read flash request
        let request = build_memory_read(0x000C, 10);

        // Valid response: bytes 0-4 match
        let mut response = [0u8; PACKET_LENGTH];
        response[0] = CMD_READ_FLASH; // Command echo
        response[1] = 0x00; // Status (success)
        response[2] = 0x00; // Address high
        response[3] = 0x0C; // Address low
        response[4] = 10; // Length
        assert!(validate_response(&request, &response));

        // Invalid response: different address
        let mut bad_response = response;
        bad_response[3] = 0x10; // Different address
        assert!(!validate_response(&request, &bad_response));

        // Invalid response: different length
        let mut bad_response2 = response;
        bad_response2[4] = 8; // Different length
        assert!(!validate_response(&request, &bad_response2));
    }

    #[test]
    fn test_validate_response_other_commands() {
        // Create a battery command request
        let request = build_simple_command(CMD_BATTERY);

        // Valid response: command matches
        let mut response = [0u8; PACKET_LENGTH];
        response[0] = CMD_BATTERY; // Command echo
        response[1] = 0x00; // Status (success)
        assert!(validate_response(&request, &response));

        // Invalid response: different command
        let mut bad_response = response;
        bad_response[0] = CMD_READ_FLASH;
        assert!(!validate_response(&request, &bad_response));
    }

    #[test]
    fn test_parse_status_changed_notification() {
        // Valid StatusChanged notification with Report ID
        let mut packet = [0u8; PACKET_LENGTH];
        packet[0] = REPORT_ID; // Report ID
        packet[1] = CMD_STATUS_CHANGED; // 0x0A
        packet[2] = 0x00; // Status
        packet[6] = 0x43; // Change flags: DPI + Report Rate + Battery

        let flags = parse_status_changed_notification(&packet);
        assert!(flags.is_some());

        let flags = flags.unwrap();
        assert!(flags.dpi_changed());
        assert!(flags.report_rate_changed());
        assert!(!flags.profile_changed());
        assert!(flags.battery_changed());
    }

    #[test]
    fn test_is_status_changed_notification() {
        // Valid StatusChanged notification
        let mut packet = [0u8; PACKET_LENGTH];
        packet[0] = REPORT_ID;
        packet[1] = CMD_STATUS_CHANGED;
        assert!(is_status_changed_notification(&packet));

        // Not a StatusChanged notification
        let mut other_packet = [0u8; PACKET_LENGTH];
        other_packet[0] = REPORT_ID;
        other_packet[1] = CMD_BATTERY;
        assert!(!is_status_changed_notification(&other_packet));

        // Empty packet
        let short_packet = [0u8; 1];
        assert!(!is_status_changed_notification(&short_packet));
    }
}
