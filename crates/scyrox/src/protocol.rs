//! USB protocol constants and packet building functions.

// =============================================================================
// USB Device Identifiers
// =============================================================================

/// Vendor ID for the mouse.
pub const VENDOR_ID: u16 = 0x3554;

/// Product ID for wired mode.
pub const PID_WIRED: u16 = 0xf5f6;

/// Product ID for wireless mode.
pub const PID_WIRELESS: u16 = 0xf5f7;

/// Supported Product IDs (preferred first).
pub const PRODUCT_IDS: [u16; 2] = [PID_WIRED, PID_WIRELESS];

/// USB interface number for configuration.
pub const INTERFACE_NUM: u8 = 1;

/// Interrupt endpoint for reading responses.
pub const INTERRUPT_EP_IN: u8 = 0x82;

/// Packet size for wired mode (from USB descriptors).
pub const PACKET_SIZE_WIRED: usize = 64;

/// Packet size for wireless mode (from USB descriptors).
pub const PACKET_SIZE_WIRELESS: usize = 49;

// =============================================================================
// Command Codes
// =============================================================================

/// Device info/serial command.
pub const CMD_DEVICE_INFO: u8 = 0x01;

/// Config flags command.
pub const CMD_CONFIG_FLAGS: u8 = 0x02;

/// Status command.
pub const CMD_STATUS: u8 = 0x03;

/// Battery status command.
pub const CMD_BATTERY: u8 = 0x04;

/// Memory write command.
pub const CMD_MEMORY_WRITE: u8 = 0x07;

/// Memory read command.
pub const CMD_MEMORY_READ: u8 = 0x08;

/// Mouse firmware version command.
pub const CMD_MOUSE_FIRMWARE: u8 = 0x12;

/// Long distance mode command (special, not memory write).
pub const CMD_LONG_DISTANCE: u8 = 0x16;

/// Wireless status command.
pub const CMD_WIRELESS_STATUS: u8 = 0x17;

/// Receiver firmware version command.
pub const CMD_RECEIVER_FIRMWARE: u8 = 0x1d;

// =============================================================================
// Memory Offsets (for read/write commands)
// =============================================================================

/// Polling rate (1 byte).
pub const OFFSET_POLLING_RATE: u16 = 0x0000;

/// Lift-off distance (1 byte).
pub const OFFSET_LIFT_OFF_DISTANCE: u16 = 0x000A;

/// Sleep timeout in units of 10 seconds (1 byte).
pub const OFFSET_SLEEP_TIMEOUT: u16 = 0x00AD;

/// Angle snapping on/off (1 byte).
pub const OFFSET_ANGLE_SNAPPING: u16 = 0x00AF;

/// Ripple control on/off (1 byte).
pub const OFFSET_RIPPLE_CONTROL: u16 = 0x00B1;

/// High speed mode on/off (1 byte).
pub const OFFSET_HIGH_SPEED_MODE: u16 = 0x00B5;

/// Sleep timeout secondary location (must be written alongside OFFSET_SLEEP_TIMEOUT).
pub const OFFSET_SLEEP_TIMEOUT_SECONDARY: u16 = 0x00B7;

// =============================================================================
// Checksum Functions
// =============================================================================

/// Calculate packet checksum.
///
/// The checksum makes the sum of all 17 bytes equal 0x55.
/// Formula: checksum = 0x55 - sum(bytes 0..16)
pub fn calculate_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    0x55u8.wrapping_sub(sum)
}

/// Calculate data checksum for write commands.
///
/// The data checksum makes the sum of all data bytes equal 0x55.
/// Formula: checksum = 0x55 - sum(data bytes)
pub fn calculate_data_checksum(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    0x55u8.wrapping_sub(sum)
}

// =============================================================================
// Packet Building Functions
// =============================================================================

/// Build a generic command packet (17 bytes).
///
/// Packet format:
/// - Byte 0: 0x08 (header)
/// - Byte 1: Command code
/// - Byte 2: Sub-command
/// - Bytes 3-4: Offset (big-endian)
/// - Byte 5: Length/parameter
/// - Bytes 6-15: Data (zeroed by default)
/// - Byte 16: Checksum
pub fn build_command(cmd: u8, subcmd: u8, offset: u16, length: u8) -> [u8; 17] {
    let mut packet = [0u8; 17];
    packet[0] = 0x08;
    packet[1] = cmd;
    packet[2] = subcmd;
    packet[3] = (offset >> 8) as u8; // Offset HIGH byte
    packet[4] = offset as u8; // Offset LOW byte
    packet[5] = length;
    // Bytes 6-15 are zero
    packet[16] = calculate_checksum(&packet[0..16]);
    packet
}

/// Build memory read command (cmd 0x08).
pub fn build_memory_read(offset: u16, length: u8) -> [u8; 17] {
    build_command(CMD_MEMORY_READ, 0x00, offset, length)
}

/// Build memory write command (cmd 0x07) for a single byte value.
///
/// Write format uses 2 data bytes: value + data checksum.
pub fn build_memory_write(offset: u16, value: u8) -> [u8; 17] {
    let mut packet = [0u8; 17];
    packet[0] = 0x08;
    packet[1] = CMD_MEMORY_WRITE;
    packet[2] = 0x00;
    packet[3] = (offset >> 8) as u8;
    packet[4] = offset as u8;
    packet[5] = 0x02; // Length: value byte + data checksum byte
    packet[6] = value;
    packet[7] = calculate_data_checksum(&[value]);
    packet[16] = calculate_checksum(&packet[0..16]);
    packet
}

/// Build device info command (cmd 0x01).
pub fn build_device_info_cmd(offset: u16, length: u8) -> [u8; 17] {
    build_command(CMD_DEVICE_INFO, 0x00, offset, length)
}

/// Build status command (cmd 0x03).
pub fn build_status_cmd() -> [u8; 17] {
    build_command(CMD_STATUS, 0x00, 0, 0)
}

/// Build battery status command (cmd 0x04).
pub fn build_battery_cmd() -> [u8; 17] {
    build_command(CMD_BATTERY, 0x00, 0, 0)
}

/// Build mouse firmware version command (cmd 0x12).
pub fn build_mouse_firmware_cmd() -> [u8; 17] {
    build_command(CMD_MOUSE_FIRMWARE, 0x00, 0, 0)
}

/// Build receiver firmware version command (cmd 0x1d).
pub fn build_receiver_firmware_cmd() -> [u8; 17] {
    build_command(CMD_RECEIVER_FIRMWARE, 0x00, 0, 0)
}

/// Build wireless status command (cmd 0x17).
pub fn build_wireless_status_cmd() -> [u8; 17] {
    build_command(CMD_WIRELESS_STATUS, 0x00, 0, 0)
}

/// Build config flags command (cmd 0x02).
pub fn build_config_flags_cmd(param: u16) -> [u8; 17] {
    build_command(CMD_CONFIG_FLAGS, 0x00, param, 0)
}

/// Build long distance mode command (cmd 0x16).
///
/// This is a special command that doesn't use the memory write format.
pub fn build_long_distance_cmd(enabled: bool) -> [u8; 17] {
    let mut packet = [0u8; 17];
    packet[0] = 0x08;
    packet[1] = CMD_LONG_DISTANCE;
    packet[2] = 0x00;
    packet[3] = 0x00;
    packet[4] = 0x00;
    packet[5] = 0x0a; // Command-specific parameter
    packet[6] = if enabled { 0x01 } else { 0x00 };
    packet[16] = calculate_checksum(&packet[0..16]);
    packet
}

// =============================================================================
// Response Parsing Utilities
// =============================================================================

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
pub fn voltage_to_percentage(voltage_mv: u16) -> u8 {
    // Formula: p = 123 - 123 / (1 + (v/3.7)^80)^0.165
    let v = voltage_mv as f32 / 1000.0;
    let denom = (1.0 + (v / 3.7).powi(80)).powf(0.165);
    let value = 123.0 - 123.0 / denom;
    let percent = value.round() as i32;
    percent.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_calculation() {
        // Test with known good packet from dump: set polling rate 125Hz
        // 08 07 00 00 00 02 08 4d 00 00 00 00 00 00 00 00 ef
        let packet = [
            0x08, 0x07, 0x00, 0x00, 0x00, 0x02, 0x08, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        assert_eq!(calculate_checksum(&packet), 0xef);
    }

    #[test]
    fn test_data_checksum_calculation() {
        // Data byte 0x08 should have checksum 0x4d (0x08 + 0x4d = 0x55)
        assert_eq!(calculate_data_checksum(&[0x08]), 0x4d);
        // Data byte 0x01 should have checksum 0x54
        assert_eq!(calculate_data_checksum(&[0x01]), 0x54);
        // Data byte 0x00 should have checksum 0x55
        assert_eq!(calculate_data_checksum(&[0x00]), 0x55);
    }

    #[test]
    fn test_build_memory_write() {
        // Test building a polling rate write command for 125Hz
        let packet = build_memory_write(OFFSET_POLLING_RATE, 0x08);
        assert_eq!(packet[0], 0x08); // Header
        assert_eq!(packet[1], CMD_MEMORY_WRITE); // Command
        assert_eq!(packet[3], 0x00); // Offset high
        assert_eq!(packet[4], 0x00); // Offset low
        assert_eq!(packet[5], 0x02); // Length
        assert_eq!(packet[6], 0x08); // Value
        assert_eq!(packet[7], 0x4d); // Data checksum
        assert_eq!(packet[16], 0xef); // Packet checksum
    }

    #[test]
    fn test_build_long_distance_cmd() {
        // Enable long distance mode
        let packet = build_long_distance_cmd(true);
        assert_eq!(packet[0], 0x08);
        assert_eq!(packet[1], CMD_LONG_DISTANCE);
        assert_eq!(packet[5], 0x0a);
        assert_eq!(packet[6], 0x01);
        assert_eq!(packet[16], 0x2c); // Known checksum from dump

        // Disable long distance mode
        let packet = build_long_distance_cmd(false);
        assert_eq!(packet[6], 0x00);
        assert_eq!(packet[16], 0x2d); // Known checksum from dump
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
}
